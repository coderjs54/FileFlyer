use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 持久化到本地的用户设置
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 设备唯一标识，首次启动时生成
    pub device_id: String,
    /// 展示给局域网内其他设备的名称
    pub device_name: String,
    /// 是否自动接收文件（无需弹窗确认）
    pub auto_accept: bool,
    /// 接收文件的保存目录
    pub download_dir: Option<String>,
}

/// 返回给前端展示的设置信息
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub auto_accept: bool,
    pub download_dir: String,
    pub tcp_port: u16,
    pub app_version: String,
}

/// 局域网内发现的设备
#[derive(Clone)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub addr: SocketAddr,
    pub last_seen: Instant,
}

/// 返回给前端的附近设备
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub ip: String,
    pub port: u16,
}

/// 单个文件的传输状态
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferFile {
    pub name: String,
    pub size: u64,
    pub transferred: u64,
    pub status: String,
}

/// 一次传输任务（可包含多个文件）
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub id: String,
    /// send | receive
    pub direction: String,
    pub peer_id: String,
    pub peer_name: String,
    pub files: Vec<TransferFile>,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    /// preparing | connecting | waiting | transferring | verifying | completed | failed | cancelled | declined
    pub status: String,
    pub speed: u64,
    pub error: Option<String>,
    pub created_at: u64,
    /// 接收完成后的保存目录
    pub dir: Option<String>,
}

impl Transfer {
    /// 是否为不可再变化的状态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "completed" | "failed" | "cancelled" | "declined"
        )
    }
}

/// 接收端待用户确认的请求
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequest {
    pub id: String,
    pub peer_name: String,
    pub files: Vec<IncomingFile>,
    pub total_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingFile {
    pub name: String,
    pub size: u64,
}

/// 前端初始化时一次性获取的完整快照
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: SettingsView,
    pub peers: Vec<PeerView>,
    pub transfers: Vec<Transfer>,
}

/// 应用全局共享状态
pub struct AppState {
    pub settings: RwLock<Settings>,
    pub peers: RwLock<HashMap<String, Peer>>,
    pub transfers: RwLock<HashMap<String, Transfer>>,
    /// 等待用户确认的接收请求，值为回传确认结果的通道
    pub pending_incoming: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    /// 每个传输任务的取消令牌
    pub cancel_tokens: Mutex<HashMap<String, CancellationToken>>,
    pub tcp_port: u16,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn settings_snapshot(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn settings_view(&self) -> SettingsView {
        let settings = self.settings_snapshot();
        SettingsView {
            device_id: settings.device_id,
            device_name: settings.device_name,
            platform: platform().to_string(),
            auto_accept: settings.auto_accept,
            download_dir: settings.download_dir.unwrap_or_default(),
            tcp_port: self.tcp_port,
            app_version: APP_VERSION.to_string(),
        }
    }

    pub fn peer_views(&self) -> Vec<PeerView> {
        let peers = self.peers.read().unwrap();
        let mut list: Vec<PeerView> = peers
            .values()
            .map(|peer| PeerView {
                id: peer.id.clone(),
                name: peer.name.clone(),
                platform: peer.platform.clone(),
                ip: peer.addr.ip().to_string(),
                port: peer.addr.port(),
            })
            .collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        list
    }

    pub fn transfer_list(&self) -> Vec<Transfer> {
        let transfers = self.transfers.read().unwrap();
        let mut list: Vec<Transfer> = transfers.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }
}

/// 当前运行平台标识，用于前端选择对应图标
pub fn platform() -> &'static str {
    std::env::consts::OS
}
