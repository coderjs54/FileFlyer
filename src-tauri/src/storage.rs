use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::state::{AppState, Settings, SettingsView};

/// 各平台默认的文件保存目录
pub fn default_download_dir(app: &AppHandle) -> PathBuf {
    // 桌面端优先使用系统“下载”目录
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Ok(dir) = app.path().download_dir() {
            return dir.join("FileFlyer");
        }
    }
    // 移动端使用应用私有目录，避免系统分区存储权限限制
    app.path()
        .document_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("FileFlyer")
}

/// 首次启动时的设备名称
pub fn default_device_name() -> String {
    if cfg!(target_os = "android") {
        return "Android 设备".to_string();
    }
    if cfg!(target_os = "ios") {
        return "iPhone / iPad".to_string();
    }
    hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && name != "localhost")
        .unwrap_or_else(|| "我的设备".to_string())
}

/// 读取设置；文件不存在或解析失败时生成一份可用配置
pub fn load_settings(app: &AppHandle, config_path: &Path) -> Settings {
    let mut settings = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
        .unwrap_or_default();

    if settings.device_id.trim().is_empty() {
        settings.device_id = uuid::Uuid::new_v4().to_string();
    }
    if settings.device_name.trim().is_empty() {
        settings.device_name = default_device_name();
    }
    if settings
        .download_dir
        .as_deref()
        .map(|dir| dir.trim().is_empty())
        .unwrap_or(true)
    {
        settings.download_dir = Some(default_download_dir(app).to_string_lossy().to_string());
    }
    if let Some(dir) = &settings.download_dir {
        std::fs::create_dir_all(dir).ok();
    }
    let _ = persist_settings(config_path, &settings);
    settings
}

/// 将设置写入磁盘
pub fn persist_settings(config_path: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("序列化设置失败：{err}"))?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("创建配置目录失败：{err}"))?;
    }
    std::fs::write(config_path, json).map_err(|err| format!("保存设置失败：{err}"))
}

/// 更新设备名称、自动接收开关与保存目录
#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    device_name: String,
    auto_accept: bool,
    download_dir: Option<String>,
) -> Result<SettingsView, String> {
    let name = device_name.trim().to_string();
    if name.is_empty() {
        return Err("设备名称不能为空".to_string());
    }
    if name.chars().count() > 40 {
        return Err("设备名称不能超过 40 个字符".to_string());
    }

    let state = app.state::<AppState>();
    let mut settings = state.settings_snapshot();
    settings.device_name = name;
    settings.auto_accept = auto_accept;

    // 传入 None 表示保持原保存目录不变
    if let Some(dir) = download_dir {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            std::fs::create_dir_all(&path).map_err(|err| format!("无法创建保存目录：{err}"))?;
            settings.download_dir = Some(path.to_string_lossy().to_string());
        }
    }

    persist_settings(&state.config_path, &settings)?;
    *state.settings.write().unwrap() = settings;
    Ok(state.settings_view())
}
