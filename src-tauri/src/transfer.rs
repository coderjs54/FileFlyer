use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::files::SourceFile;
use crate::protocol::{
    read_frame, write_chunk, write_control, Control, FileMeta, Frame, CHUNK_SIZE,
};
use crate::state::{AppState, IncomingFile, IncomingRequest, Peer, Transfer, TransferFile};

/// 单次传输最多包含的文件数量，避免异常的批量请求
const MAX_FILES: usize = 200;
/// 分片传输阶段的进度推送间隔
const PROGRESS_INTERVAL: Duration = Duration::from_millis(180);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// 传输任务被中断的原因
enum Abort {
    Cancelled,
    Declined(Option<String>),
    Failed(String),
}

impl From<String> for Abort {
    fn from(message: String) -> Self {
        Abort::Failed(message)
    }
}

/// 传输结束后的清理逻辑：移除取消令牌与待确认通道
struct TransferGuard {
    app: AppHandle,
    id: String,
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        let state = self.app.state::<AppState>();
        if let Ok(mut tokens) = state.cancel_tokens.lock() {
            tokens.remove(&self.id);
        };
        if let Ok(mut pending) = state.pending_incoming.lock() {
            pending.remove(&self.id);
        };
    }
}

fn register_cancel(state: &AppState, id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    state
        .cancel_tokens
        .lock()
        .unwrap()
        .insert(id.to_string(), token.clone());
    token
}

/// 按需节流地把传输状态推送给前端
struct ProgressEmitter {
    last_emit: Instant,
    last_bytes: u64,
}

impl ProgressEmitter {
    fn new() -> Self {
        Self {
            last_emit: Instant::now(),
            last_bytes: 0,
        }
    }

    fn push(&mut self, app: &AppHandle, transfer: &mut Transfer, force: bool) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_emit).as_secs_f64();

        if force {
            if transfer.is_terminal() {
                transfer.speed = 0;
            }
            store_transfer(app, transfer, true);
            self.last_emit = now;
            self.last_bytes = transfer.transferred_bytes;
        } else if elapsed >= PROGRESS_INTERVAL.as_secs_f64() {
            let delta = transfer
                .transferred_bytes
                .saturating_sub(self.last_bytes) as f64;
            let instant = (delta / elapsed.max(0.001)) as u64;
            transfer.speed = if transfer.speed == 0 {
                instant
            } else {
                (transfer.speed as f64 * 0.65 + instant as f64 * 0.35) as u64
            };
            store_transfer(app, transfer, true);
            self.last_emit = now;
            self.last_bytes = transfer.transferred_bytes;
        }
    }
}

/// 保存传输状态并广播给前端
fn store_transfer(app: &AppHandle, transfer: &Transfer, overwrite: bool) -> bool {
    let state = app.state::<AppState>();
    {
        let mut transfers = state.transfers.write().unwrap();
        if !overwrite && transfers.contains_key(&transfer.id) {
            return false;
        }
        transfers.insert(transfer.id.clone(), transfer.clone());

        // 控制历史记录数量，只清理已结束的旧任务
        if transfers.len() > 80 {
            let mut finished: Vec<(u64, String)> = transfers
                .values()
                .filter(|item| item.is_terminal())
                .map(|item| (item.created_at, item.id.clone()))
                .collect();
            finished.sort_by_key(|(created_at, _)| *created_at);
            let excess = transfers.len().saturating_sub(60);
            for (_, id) in finished.into_iter().take(excess) {
                transfers.remove(&id);
            }
        }
    }
    let _ = app.emit("transfer-updated", transfer);
    true
}

fn mark_finished(transfer: &mut Transfer, result: Result<(), Abort>) {
    transfer.speed = 0;
    match result {
        Ok(()) => {
            transfer.status = "completed".into();
            transfer.error = None;
            transfer.transferred_bytes = transfer.total_bytes;
            for file in &mut transfer.files {
                file.transferred = file.size;
                file.status = "done".into();
            }
        }
        Err(Abort::Cancelled) => {
            transfer.status = "cancelled".into();
        }
        Err(Abort::Declined(reason)) => {
            transfer.status = "declined".into();
            transfer.error = reason;
        }
        Err(Abort::Failed(message)) => {
            transfer.status = "failed".into();
            transfer.error = Some(message);
        }
    }
}

/// 启动 TCP 传输服务，端口由系统分配并通过 UDP 广播告知其他设备
pub fn spawn_server(app: AppHandle, listener: std::net::TcpListener) {
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("[transfer] TCP 服务启动失败：{err}");
                return;
            }
        };
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    eprintln!("[transfer] 收到来自 {addr} 的连接");
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        handle_connection(app, stream).await;
                    });
                }
                Err(err) => {
                    eprintln!("[transfer] 接受连接失败：{err}");
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
            }
        }
    });
}

/// 发送方：把文件加入发送队列并立即返回传输 ID
#[tauri::command]
pub async fn send_files(
    app: AppHandle,
    peer_id: String,
    paths: Vec<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("请选择要发送的文件".to_string());
    }
    if paths.len() > MAX_FILES {
        return Err(format!("一次最多发送 {MAX_FILES} 个文件"));
    }

    let peer = {
        let state = app.state::<AppState>();
        let peers = state.peers.read().unwrap();
        peers
            .get(&peer_id)
            .cloned()
            .ok_or_else(|| "设备已离线，请等待重新发现后再试".to_string())?
    };

    let mut files = Vec::with_capacity(paths.len());
    let mut sources = Vec::with_capacity(paths.len());
    for raw in &paths {
        let source = crate::files::describe_source(&app, raw)?;
        files.push(TransferFile {
            name: source.name.clone(),
            size: source.size,
            transferred: 0,
            status: "pending".into(),
        });
        sources.push(source);
    }

    let total_bytes = files.iter().map(|file| file.size).sum();
    let peer_name = peer.name.clone();

    let id = uuid::Uuid::new_v4().to_string();
    let transfer = Transfer {
        id: id.clone(),
        direction: "send".into(),
        peer_id: peer.id.clone(),
        peer_name,
        files,
        total_bytes,
        transferred_bytes: 0,
        status: "preparing".into(),
        speed: 0,
        error: None,
        created_at: now_ms(),
        dir: None,
    };
    store_transfer(&app, &transfer, true);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        run_sender(handle, transfer, peer, sources).await;
    });
    Ok(id)
}

async fn run_sender(app: AppHandle, mut transfer: Transfer, peer: Peer, sources: Vec<SourceFile>) {
    let state = app.state::<AppState>();
    let token = register_cancel(&state, &transfer.id);
    let _guard = TransferGuard {
        app: app.clone(),
        id: transfer.id.clone(),
    };
    let mut emitter = ProgressEmitter::new();

    transfer.status = "connecting".into();
    emitter.push(&app, &mut transfer, true);

    let result = send_inner(&app, &mut transfer, &peer, &sources, &token, &mut emitter).await;
    mark_finished(&mut transfer, result);
    emitter.push(&app, &mut transfer, true);
}

async fn send_inner(
    app: &AppHandle,
    transfer: &mut Transfer,
    peer: &Peer,
    sources: &[SourceFile],
    token: &CancellationToken,
    emitter: &mut ProgressEmitter,
) -> Result<(), Abort> {
    let (from_id, from_name) = {
        let state = app.state::<AppState>();
        let settings = state.settings_snapshot();
        (settings.device_id, settings.device_name)
    };

    let connect = tokio::select! {
        _ = token.cancelled() => return Err(Abort::Cancelled),
        result = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect(peer.addr)) => result,
    };
    let stream = match connect {
        Err(_) => {
            return Err(Abort::Failed(
                "连接对方设备超时，请确认双方连接同一局域网且应用已打开".into(),
            ))
        }
        Ok(Err(err)) => return Err(Abort::Failed(format!("无法连接对方设备：{err}"))),
        Ok(Ok(stream)) => stream,
    };
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    let offer = Control::Offer {
        id: transfer.id.clone(),
        from_id,
        from_name,
        files: transfer
            .files
            .iter()
            .map(|file| FileMeta {
                name: file.name.clone(),
                size: file.size,
            })
            .collect(),
    };
    write_control(&mut writer, &offer)
        .await
        .map_err(|err| Abort::Failed(format!("发送传输请求失败：{err}")))?;

    transfer.status = "waiting".into();
    emitter.push(app, transfer, true);

    let reply = wait_control(&mut reader, token, Duration::from_secs(180)).await?;
    match reply {
        Control::Accept => {}
        Control::Reject { reason } => return Err(Abort::Declined(reason)),
        Control::Cancel { .. } => return Err(Abort::Cancelled),
        _ => return Err(Abort::Failed("对方返回了无法识别的响应".into())),
    }

    transfer.status = "transferring".into();
    emitter.push(app, transfer, true);

    for index in 0..sources.len() {
        if token.is_cancelled() {
            let _ = write_control(
                &mut writer,
                &Control::Cancel {
                    reason: Some("发送方已取消".into()),
                },
            )
            .await;
            return Err(Abort::Cancelled);
        }

        transfer.files[index].status = "transferring".into();
        transfer.status = "transferring".into();
        emitter.push(app, transfer, true);

        let source = &sources[index];
        let std_file = crate::files::open_source(app, &source.raw).map_err(|err| {
            Abort::Failed(format!("无法打开文件「{}」：{err}", source.name))
        })?;
        let mut file = tokio::fs::File::from_std(std_file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; CHUNK_SIZE];

        loop {
            if token.is_cancelled() {
                let _ = write_control(
                    &mut writer,
                    &Control::Cancel {
                        reason: Some("发送方已取消".into()),
                    },
                )
                .await;
                return Err(Abort::Cancelled);
            }
            let read = match file.read(&mut buffer).await {
                Ok(0) => break,
                Ok(read) => read,
                Err(err) => return Err(Abort::Failed(format!("读取文件失败：{err}"))),
            };
            hasher.update(&buffer[..read]);
            write_chunk(&mut writer, index as u32, &buffer[..read])
                .await
                .map_err(|err| Abort::Failed(format!("传输中断：{err}")))?;
            transfer.files[index].transferred += read as u64;
            transfer.transferred_bytes += read as u64;
            emitter.push(app, transfer, false);
        }

        // 读取完成即释放文件句柄，并结束 iOS 的安全作用域访问
        drop(file);
        crate::files::close_source(app, &source.raw);

        if transfer.files[index].transferred != transfer.files[index].size {
            return Err(Abort::Failed(
                "文件在发送过程中被修改，传输已停止".to_string(),
            ));
        }

        transfer.files[index].status = "verifying".into();
        transfer.status = "verifying".into();
        emitter.push(app, transfer, true);

        let digest = hex::encode(hasher.finalize());
        write_control(
            &mut writer,
            &Control::FileComplete {
                index,
                sha256: digest,
            },
        )
        .await
        .map_err(|err| Abort::Failed(format!("发送文件校验信息失败：{err}")))?;

        let verified = wait_control(&mut reader, token, Duration::from_secs(300)).await?;
        match verified {
            Control::FileVerified { index: verified_index, ok } if verified_index == index => {
                if !ok {
                    return Err(Abort::Failed(format!(
                        "文件「{}」校验失败，接收方已丢弃",
                        transfer.files[index].name
                    )));
                }
                transfer.files[index].status = "done".into();
            }
            Control::Cancel { .. } => return Err(Abort::Cancelled),
            _ => return Err(Abort::Failed("对方返回了无法识别的校验结果".into())),
        }
        emitter.push(app, transfer, true);
    }

    write_control(
        &mut writer,
        &Control::Finish {
            ok: true,
            error: None,
        },
    )
    .await
    .map_err(|err| Abort::Failed(format!("发送完成通知失败：{err}")))?;

    transfer.status = "verifying".into();
    emitter.push(app, transfer, true);

    let result = wait_control(&mut reader, token, Duration::from_secs(120)).await?;
    match result {
        Control::Result { ok: true, .. } => Ok(()),
        Control::Result { ok: false, error } => {
            Err(Abort::Failed(error.unwrap_or_else(|| "对方未能完成文件保存".into())))
        }
        Control::Cancel { .. } => Err(Abort::Cancelled),
        _ => Err(Abort::Failed("未收到接收方的完成确认".into())),
    }
}

/// 等待一条控制消息，期间响应取消与超时
async fn wait_control(
    reader: &mut OwnedReadHalf,
    token: &CancellationToken,
    timeout: Duration,
) -> Result<Control, Abort> {
    let wait = async {
        loop {
            match read_frame(reader).await {
                Ok(Frame::Control(control)) => return Ok(control),
                Ok(Frame::Chunk { .. }) => continue,
                Err(err) => {
                    return Err(Abort::Failed(format!("与对方设备的连接已断开：{err}")))
                }
            }
        }
    };

    tokio::select! {
        _ = token.cancelled() => Err(Abort::Cancelled),
        result = tokio::time::timeout(timeout, wait) => match result {
            Ok(value) => value,
            Err(_) => Err(Abort::Failed("等待对方响应超时".into())),
        },
    }
}

/// 接收方：处理一次 TCP 连接
async fn handle_connection(app: AppHandle, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    let first = tokio::time::timeout(Duration::from_secs(10), read_frame(&mut reader)).await;
    let (id, from_id, from_name, files_meta) = match first {
        Ok(Ok(Frame::Control(Control::Offer {
            id,
            from_id,
            from_name,
            files,
        }))) => (id, from_id, from_name, files),
        Ok(Ok(_)) => return,
        Ok(Err(err)) => {
            eprintln!("[transfer] 读取传输请求失败：{err}");
            return;
        }
        Err(_) => return,
    };

    if id.trim().is_empty() || files_meta.is_empty() || files_meta.len() > MAX_FILES {
        return;
    }

    let files_meta: Vec<FileMeta> = files_meta
        .into_iter()
        .map(|file| FileMeta {
            name: sanitize_file_name(&file.name),
            size: file.size,
        })
        .collect();
    let total_bytes = files_meta
        .iter()
        .fold(0u64, |total, file| total.saturating_add(file.size));

    let mut transfer = Transfer {
        id: id.clone(),
        direction: "receive".into(),
        peer_id: from_id,
        peer_name: if from_name.trim().is_empty() {
            "未知设备".to_string()
        } else {
            from_name
        },
        files: files_meta
            .iter()
            .map(|file| TransferFile {
                name: file.name.clone(),
                size: file.size,
                transferred: 0,
                status: "pending".into(),
            })
            .collect(),
        total_bytes,
        transferred_bytes: 0,
        status: "waiting".into(),
        speed: 0,
        error: None,
        created_at: now_ms(),
        dir: None,
    };

    // 同一个传输 ID 只允许处理一次
    if !store_transfer(&app, &transfer, false) {
        return;
    }

    let state = app.state::<AppState>();
    let token = register_cancel(&state, &id);
    let _guard = TransferGuard {
        app: app.clone(),
        id: id.clone(),
    };
    let auto_accept = state.settings_snapshot().auto_accept;
    drop(state);

    let mut emitter = ProgressEmitter::new();

    if !auto_accept {
        let (decision_tx, decision_rx) = oneshot::channel::<bool>();
        app.state::<AppState>()
            .pending_incoming
            .lock()
            .unwrap()
            .insert(id.clone(), decision_tx);

        let request = IncomingRequest {
            id: id.clone(),
            peer_name: transfer.peer_name.clone(),
            files: files_meta
                .iter()
                .map(|file| IncomingFile {
                    name: file.name.clone(),
                    size: file.size,
                })
                .collect(),
            total_bytes,
        };
        let _ = app.emit("incoming-request", request);

        let accepted = tokio::select! {
            decision = decision_rx => decision.unwrap_or(false),
            _ = token.cancelled() => false,
            frame = read_frame(&mut reader) => {
                // 等待确认期间对方主动取消或断开
                if let Ok(Frame::Control(Control::Cancel { reason })) = frame {
                    transfer.error = reason;
                }
                let _ = app.emit("incoming-cancelled", id.clone());
                transfer.status = "cancelled".into();
                transfer.error = Some("对方已取消发送".into());
                emitter.push(&app, &mut transfer, true);
                return;
            }
        };
        app.state::<AppState>()
            .pending_incoming
            .lock()
            .unwrap()
            .remove(&id);

        if !accepted {
            let _ = write_control(
                &mut writer,
                &Control::Reject {
                    reason: Some("对方拒绝了此次传输".into()),
                },
            )
            .await;
            if token.is_cancelled() {
                transfer.status = "cancelled".into();
            } else {
                transfer.status = "declined".into();
            }
            emitter.push(&app, &mut transfer, true);
            return;
        }
    }

    if let Err(err) = write_control(&mut writer, &Control::Accept).await {
        transfer.status = "failed".into();
        transfer.error = Some(format!("回复接收确认失败：{err}"));
        emitter.push(&app, &mut transfer, true);
        return;
    }

    let download_dir = {
        let state = app.state::<AppState>();
        let settings = state.settings_snapshot();
        settings
            .download_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::storage::default_download_dir(&app))
    };
    if let Err(err) = fs::create_dir_all(&download_dir).await {
        transfer.status = "failed".into();
        transfer.error = Some(format!("无法创建保存目录：{err}"));
        emitter.push(&app, &mut transfer, true);
        return;
    }

    let mut used_names = HashSet::new();
    let final_paths: Vec<PathBuf> = transfer
        .files
        .iter()
        .map(|file| unique_file_path(&download_dir, &file.name, &mut used_names))
        .collect();

    transfer.status = "transferring".into();
    emitter.push(&app, &mut transfer, true);

    let mut receiving: Option<ReceivingFile> = None;
    let mut index = 0usize;
    let mut completed = 0usize;

    loop {
        let frame = tokio::select! {
            _ = token.cancelled() => {
                let _ = write_control(
                    &mut writer,
                    &Control::Cancel { reason: Some("接收方已取消".into()) },
                ).await;
                if let Some(current) = receiving.take() {
                    drop(current.file);
                    let _ = fs::remove_file(&current.part).await;
                }
                transfer.status = "cancelled".into();
                emitter.push(&app, &mut transfer, true);
                return;
            }
            result = read_frame(&mut reader) => match result {
                Ok(frame) => frame,
                Err(err) => {
                    if let Some(current) = receiving.take() {
                        drop(current.file);
                        let _ = fs::remove_file(&current.part).await;
                    }
                    transfer.status = "failed".into();
                    transfer.error = Some(format!("连接已断开：{err}"));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }
            },
        };

        match frame {
            Frame::Chunk { index: chunk_index, data } => {
                if chunk_index as usize != index || index >= transfer.files.len() {
                    if let Some(current) = receiving.take() {
                        drop(current.file);
                        let _ = fs::remove_file(&current.part).await;
                    }
                    transfer.status = "failed".into();
                    transfer.error = Some("收到的文件序号异常，传输已终止".into());
                    emitter.push(&app, &mut transfer, true);
                    return;
                }
                let expected = transfer.files[index].size;
                if transfer.files[index]
                    .transferred
                    .saturating_add(data.len() as u64)
                    > expected
                {
                    if let Some(current) = receiving.take() {
                        drop(current.file);
                        let _ = fs::remove_file(&current.part).await;
                    }
                    transfer.status = "failed".into();
                    transfer.error = Some("收到的数据超过文件声明大小，传输已终止".into());
                    emitter.push(&app, &mut transfer, true);
                    return;
                }

                if receiving.is_none() {
                    let part = part_path(&download_dir, &final_paths[index], &transfer.id);
                    match File::create(&part).await {
                        Ok(file) => {
                            receiving = Some(ReceivingFile {
                                file,
                                hasher: Sha256::new(),
                                part,
                            });
                            transfer.files[index].status = "transferring".into();
                            emitter.push(&app, &mut transfer, true);
                        }
                        Err(err) => {
                            transfer.status = "failed".into();
                            transfer.error = Some(format!("无法创建文件：{err}"));
                            emitter.push(&app, &mut transfer, true);
                            return;
                        }
                    }
                }

                let current = receiving.as_mut().unwrap();
                if let Err(err) = current.file.write_all(&data).await {
                    transfer.status = "failed".into();
                    transfer.error = Some(format!("写入文件失败：{err}"));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }
                current.hasher.update(&data);
                transfer.files[index].transferred += data.len() as u64;
                transfer.transferred_bytes += data.len() as u64;
                emitter.push(&app, &mut transfer, false);
            }
            Frame::Control(Control::FileComplete {
                index: complete_index,
                sha256,
            }) => {
                if complete_index != index || index >= transfer.files.len() {
                    if let Some(current) = receiving.take() {
                        drop(current.file);
                        let _ = fs::remove_file(&current.part).await;
                    }
                    transfer.status = "failed".into();
                    transfer.error = Some("收到的完成通知序号异常，传输已终止".into());
                    emitter.push(&app, &mut transfer, true);
                    return;
                }

                // 空文件不会收到任何数据分片，需要在这里补建
                if receiving.is_none() {
                    if transfer.files[index].size != 0 {
                        transfer.status = "failed".into();
                        transfer.error = Some("文件数据不完整，传输已终止".into());
                        emitter.push(&app, &mut transfer, true);
                        return;
                    }
                    let part = part_path(&download_dir, &final_paths[index], &transfer.id);
                    match File::create(&part).await {
                        Ok(file) => {
                            receiving = Some(ReceivingFile {
                                file,
                                hasher: Sha256::new(),
                                part,
                            });
                        }
                        Err(err) => {
                            transfer.status = "failed".into();
                            transfer.error = Some(format!("无法创建文件：{err}"));
                            emitter.push(&app, &mut transfer, true);
                            return;
                        }
                    }
                }

                let mut current = receiving.take().unwrap();
                if let Err(err) = current.file.flush().await {
                    let _ = fs::remove_file(&current.part).await;
                    transfer.status = "failed".into();
                    transfer.error = Some(format!("保存文件失败：{err}"));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }
                drop(current.file);

                let actual = hex::encode(current.hasher.finalize());
                let size_ok = transfer.files[index].transferred == transfer.files[index].size;
                let verified = size_ok && actual.eq_ignore_ascii_case(&sha256);

                if !verified {
                    let _ = fs::remove_file(&current.part).await;
                    let _ = write_control(
                        &mut writer,
                        &Control::FileVerified {
                            index,
                            ok: false,
                        },
                    )
                    .await;
                    transfer.status = "failed".into();
                    transfer.error = Some(format!(
                        "文件「{}」校验失败，已丢弃",
                        transfer.files[index].name
                    ));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }

                if let Err(err) = fs::rename(&current.part, &final_paths[index]).await {
                    let _ = fs::remove_file(&current.part).await;
                    let _ = write_control(
                        &mut writer,
                        &Control::Cancel {
                            reason: Some("接收方保存文件失败".into()),
                        },
                    )
                    .await;
                    transfer.status = "failed".into();
                    transfer.error = Some(format!("保存文件失败：{err}"));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }

                if let Err(err) = write_control(
                    &mut writer,
                    &Control::FileVerified { index, ok: true },
                )
                .await
                {
                    transfer.status = "failed".into();
                    transfer.error = Some(format!("回复校验结果失败：{err}"));
                    emitter.push(&app, &mut transfer, true);
                    return;
                }

                transfer.files[index].status = "done".into();
                transfer.files[index].transferred = transfer.files[index].size;
                completed += 1;
                index += 1;
                emitter.push(&app, &mut transfer, true);
            }
            Frame::Control(Control::Finish { ok, error }) => {
                if ok && completed == transfer.files.len() {
                    transfer.status = "completed".into();
                    transfer.error = None;
                    transfer.transferred_bytes = transfer.total_bytes;
                    transfer.dir = Some(download_dir.to_string_lossy().to_string());
                    emitter.push(&app, &mut transfer, true);
                    let _ = write_control(
                        &mut writer,
                        &Control::Result {
                            ok: true,
                            error: None,
                        },
                    )
                    .await;
                } else {
                    transfer.status = "failed".into();
                    transfer.error = Some(
                        error.unwrap_or_else(|| "文件未全部接收完成".to_string()),
                    );
                    emitter.push(&app, &mut transfer, true);
                    let _ = write_control(
                        &mut writer,
                        &Control::Result {
                            ok: false,
                            error: transfer.error.clone(),
                        },
                    )
                    .await;
                }
                return;
            }
            Frame::Control(Control::Cancel { reason }) => {
                if let Some(current) = receiving.take() {
                    drop(current.file);
                    let _ = fs::remove_file(&current.part).await;
                }
                transfer.status = "cancelled".into();
                transfer.error = reason;
                emitter.push(&app, &mut transfer, true);
                return;
            }
            Frame::Control(_) => {}
        }
    }
}

/// 接收中的临时文件句柄
struct ReceivingFile {
    file: File,
    hasher: Sha256,
    part: PathBuf,
}

/// 接收端取消传输
#[tauri::command]
pub fn cancel_transfer(app: AppHandle, id: String) {
    let state = app.state::<AppState>();
    let pending = state.pending_incoming.lock().unwrap().remove(&id);
    if let Some(sender) = pending {
        let _ = sender.send(false);
    }
    let tokens = state.cancel_tokens.lock().unwrap();
    if let Some(token) = tokens.get(&id) {
        token.cancel();
    }
}

/// 接收端确认或拒绝文件
#[tauri::command]
pub fn respond_incoming(app: AppHandle, id: String, accept: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let sender = state.pending_incoming.lock().unwrap().remove(&id);
    match sender {
        Some(sender) => {
            sender
                .send(accept)
                .map_err(|_| "该传输请求已失效".to_string())
        }
        None => Err("该传输请求已失效".to_string()),
    }
}

/// 清除已结束的传输记录，返回剩余记录
#[tauri::command]
pub fn clear_finished(app: AppHandle) -> Vec<Transfer> {
    let state = app.state::<AppState>();
    {
        let mut transfers = state.transfers.write().unwrap();
        transfers.retain(|_, transfer| !transfer.is_terminal());
    }
    state.transfer_list()
}

/// 在系统文件管理器中显示接收目录
#[tauri::command]
pub fn open_download_dir(app: AppHandle, path: Option<String>) -> Result<(), String> {
    let dir = match path.filter(|value| !value.trim().is_empty()) {
        Some(value) => PathBuf::from(value),
        None => {
            let state = app.state::<AppState>();
            state
                .settings_snapshot()
                .download_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| crate::storage::default_download_dir(&app))
        }
    };
    std::fs::create_dir_all(&dir).map_err(|err| format!("无法访问保存目录：{err}"))?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .reveal_item_in_dir(&dir)
            .map_err(|err| format!("无法打开文件夹：{err}"))
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (&app, &dir);
        Err("移动端文件保存在应用目录中，暂不支持直接打开文件夹".to_string())
    }
}

/// 清理文件名中的路径分隔符、非法字符与 Windows 保留名
fn sanitize_file_name(raw: &str) -> String {
    // 手动按两种分隔符取末段，避免 Windows 下 "a:b" 被识别成盘符前缀
    let base = raw
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("file");

    let mut cleaned = String::new();
    for ch in base.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            cleaned.push('_');
        } else {
            cleaned.push(ch);
        }
    }

    let mut name = cleaned.trim().trim_matches('.').trim().to_string();
    if name.is_empty() {
        name = "file".to_string();
    }

    if name.chars().count() > 180 {
        let extension = Path::new(&name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        let stem = Path::new(&name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("file");
        let max_stem = 180usize.saturating_sub(extension.chars().count() + 1);
        let mut truncated: String = stem.chars().take(max_stem).collect();
        if truncated.trim().is_empty() {
            truncated = "file".to_string();
        }
        name = if extension.is_empty() {
            truncated
        } else {
            format!("{truncated}.{extension}")
        };
    }

    let upper = name
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        name = format!("file_{name}");
    }

    name
}

/// 生成不与已有文件、同批次文件重名的保存路径
fn unique_file_path(dir: &Path, raw_name: &str, used: &mut HashSet<String>) -> PathBuf {
    let name = sanitize_file_name(raw_name);
    let path = Path::new(&name);
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("file");
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    for i in 0..10_000 {
        let candidate = if i == 0 {
            name.clone()
        } else if extension.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{extension}")
        };
        let key = candidate.to_lowercase();
        if !used.contains(&key) && !dir.join(&candidate).exists() {
            used.insert(key);
            return dir.join(candidate);
        }
    }

    let fallback = format!(
        "{stem}-{}{}",
        now_ms(),
        if extension.is_empty() {
            String::new()
        } else {
            format!(".{extension}")
        }
    );
    used.insert(fallback.to_lowercase());
    dir.join(fallback)
}

/// 接收过程中使用的临时文件路径
fn part_path(dir: &Path, final_path: &Path, transfer_id: &str) -> PathBuf {
    let name = final_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let short: String = transfer_id.chars().take(8).collect();
    dir.join(format!(".{name}.{short}.part"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_path_and_invalid_chars() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_file_name("  ..  "), "file");
        assert_eq!(sanitize_file_name("CON.txt"), "file_CON.txt");
    }

    #[test]
    fn unique_names_avoid_conflicts() {
        let dir = std::env::temp_dir().join(format!("fileflyer-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("报告.pdf"), b"x").unwrap();

        let mut used = HashSet::new();
        let first = unique_file_path(&dir, "报告.pdf", &mut used);
        let second = unique_file_path(&dir, "报告.pdf", &mut used);
        assert_eq!(first.file_name().unwrap(), "报告 (1).pdf");
        assert_eq!(second.file_name().unwrap(), "报告 (2).pdf");

        std::fs::remove_dir_all(&dir).ok();
    }
}
