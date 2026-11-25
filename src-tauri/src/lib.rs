mod discovery;
mod files;
mod protocol;
mod state;
mod storage;
mod transfer;

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use tauri::{AppHandle, Manager};

use state::{AppState, Snapshot};

/// 前端启动时获取完整状态快照
#[tauri::command]
fn get_snapshot(app: AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    Snapshot {
        settings: state.settings_view(),
        peers: state.peer_views(),
        transfers: state.transfer_list(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();

            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            let config_path = config_dir.join("settings.json");
            let settings = storage::load_settings(&handle, &config_path);

            // 提前绑定 TCP 端口，确保广播前就能拿到真实端口
            let listener = std::net::TcpListener::bind(("0.0.0.0", 0))?;
            listener.set_nonblocking(true)?;
            let tcp_port = listener.local_addr()?.port();

            app.manage(AppState {
                settings: RwLock::new(settings),
                peers: RwLock::new(HashMap::new()),
                transfers: RwLock::new(HashMap::new()),
                pending_incoming: Mutex::new(HashMap::new()),
                cancel_tokens: Mutex::new(HashMap::new()),
                tcp_port,
                config_path,
            });

            transfer::spawn_server(handle.clone(), listener);
            discovery::spawn(handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            transfer::send_files,
            transfer::respond_incoming,
            transfer::cancel_transfer,
            transfer::clear_finished,
            transfer::open_download_dir,
            storage::update_settings,
            discovery::ping_address,
        ])
        .run(tauri::generate_context!())
        .expect("FileFlyer 启动失败");
}
