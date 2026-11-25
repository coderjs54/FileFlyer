use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::UdpSocket;
use tokio::time::{interval, MissedTickBehavior};

use crate::state::{platform, AppState, Peer};

/// 局域网发现使用的 UDP 广播端口
pub const DISCOVERY_PORT: u16 = 45731;
const APP_TAG: &str = "fileflyer";
const PROTOCOL_VERSION: u32 = 1;
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(2);
/// 超过该时间未收到广播则认为设备离线
const PEER_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Serialize, Deserialize)]
struct Announcement {
    app: String,
    v: u32,
    id: String,
    name: String,
    platform: String,
    /// 该设备的 TCP 传输服务端口
    port: u16,
}

/// 启动 UDP 广播发现服务
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(err) = run(&app).await {
                eprintln!("[discovery] {err}");
                // 端口被占用等异常场景下稍后重试
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    });
}

async fn run(app: &AppHandle) -> Result<(), String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT))
        .await
        .map_err(|err| format!("局域网发现端口绑定失败：{err}"))?;
    socket
        .set_broadcast(true)
        .map_err(|err| format!("无法开启广播：{err}"))?;
    let socket = std::sync::Arc::new(socket);

    let mut ticker = interval(ANNOUNCE_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    send_announce(app, &socket).await;

    let mut buf = vec![0u8; 4096];
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let removed = prune_peers(app);
                send_announce(app, &socket).await;
                if removed {
                    emit_peers(app);
                }
            }
            received = socket.recv_from(&mut buf) => {
                match received {
                    Ok((len, src)) => {
                        if let Ok(announcement) = serde_json::from_slice::<Announcement>(&buf[..len]) {
                            handle_announcement(app, announcement, src);
                        }
                    }
                    Err(err) => {
                        eprintln!("[discovery] 接收广播失败：{err}");
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
    }
}

/// 构造当前设备的广播内容
fn announcement_payload(app: &AppHandle) -> Option<Vec<u8>> {
    let state = app.state::<AppState>();
    let settings = state.settings_snapshot();
    serde_json::to_vec(&Announcement {
        app: APP_TAG.to_string(),
        v: PROTOCOL_VERSION,
        id: settings.device_id,
        name: settings.device_name,
        platform: platform().to_string(),
        port: state.tcp_port,
    })
    .ok()
}

async fn send_announce(app: &AppHandle, socket: &UdpSocket) {
    let Some(payload) = announcement_payload(app) else {
        return;
    };
    for target in broadcast_targets() {
        let addr = SocketAddr::new(IpAddr::V4(target), DISCOVERY_PORT);
        let _ = socket.send_to(&payload, addr).await;
    }
}

/// 收集有限广播地址与各网卡的定向广播地址
fn broadcast_targets() -> Vec<Ipv4Addr> {
    let mut targets = HashSet::new();
    targets.insert(Ipv4Addr::BROADCAST);

    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if interface.ip().is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = interface.addr {
                let mask = u32::from(v4.netmask);
                if mask == 0 {
                    continue;
                }
                let broadcast = Ipv4Addr::from(u32::from(v4.ip) | !mask);
                if !broadcast.is_broadcast() && !broadcast.is_unspecified() {
                    targets.insert(broadcast);
                }
            }
        }
    }
    targets.into_iter().collect()
}

fn handle_announcement(app: &AppHandle, announcement: Announcement, src: SocketAddr) {
    if announcement.app != APP_TAG || announcement.v != PROTOCOL_VERSION {
        return;
    }
    if announcement.port == 0 || announcement.id.trim().is_empty() {
        return;
    }
    if !src.ip().is_ipv4() || src.ip().is_loopback() {
        return;
    }

    let state = app.state::<AppState>();
    let self_id = state.settings_snapshot().device_id;
    if announcement.id == self_id {
        return;
    }

    let name = if announcement.name.trim().is_empty() {
        src.ip().to_string()
    } else {
        announcement.name.trim().to_string()
    };
    let addr = SocketAddr::new(src.ip(), announcement.port);
    let mut changed = false;

    {
        let mut peers = state.peers.write().unwrap();
        match peers.get_mut(&announcement.id) {
            Some(peer) => {
                if peer.name != name || peer.addr != addr || peer.platform != announcement.platform {
                    peer.name = name;
                    peer.addr = addr;
                    peer.platform = announcement.platform;
                    changed = true;
                }
                peer.last_seen = Instant::now();
            }
            None => {
                peers.insert(
                    announcement.id.clone(),
                    Peer {
                        id: announcement.id,
                        name,
                        platform: announcement.platform,
                        addr,
                        last_seen: Instant::now(),
                    },
                );
                changed = true;
            }
        }
    }

    if changed {
        emit_peers(app);
    }
}

/// 清理长时间未广播的设备，返回是否有设备被移除
fn prune_peers(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let mut peers = state.peers.write().unwrap();
    let before = peers.len();
    peers.retain(|_, peer| peer.last_seen.elapsed() < PEER_TIMEOUT);
    peers.len() != before
}

pub fn emit_peers(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit("peers-changed", state.peer_views());
}

/// 手动向指定 IP 发送发现包，用于广播被路由器隔离时建立连接
#[tauri::command]
pub async fn ping_address(app: AppHandle, ip: String) -> Result<(), String> {
    let target: Ipv4Addr = ip
        .trim()
        .parse()
        .map_err(|_| "请输入有效的 IPv4 地址，例如 192.168.1.10".to_string())?;
    let payload = announcement_payload(&app).ok_or_else(|| "设备信息尚未就绪".to_string())?;

    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|err| format!("无法创建连接：{err}"))?;
    socket.set_broadcast(true).ok();
    socket
        .send_to(
            &payload,
            SocketAddr::new(IpAddr::V4(target), DISCOVERY_PORT),
        )
        .await
        .map_err(|err| format!("发送连接请求失败：{err}"))?;
    Ok(())
}
