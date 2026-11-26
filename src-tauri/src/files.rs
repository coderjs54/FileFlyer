use tauri::AppHandle;
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use percent_encoding::percent_decode_str;

/// 待发送文件的来源信息
pub struct SourceFile {
    /// 展示给接收方的文件名
    pub name: String,
    pub size: u64,
    /// 前端传来的原始路径或 URI，用于真正打开文件
    pub raw: String,
}

fn read_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true);
    options
}

fn parse_path(raw: &str) -> Result<FilePath, String> {
    raw.parse::<FilePath>()
        .map_err(|_| "无效的文件路径".to_string())
}

/// 读取文件基本信息。
/// 通过 fs 插件打开文件，兼容桌面路径、iOS file:// 以及 Android content://。
pub fn describe_source(app: &AppHandle, raw: &str) -> Result<SourceFile, String> {
    let file_path = parse_path(raw)?;
    let name = display_name(&file_path, raw);
    let file = app
        .fs()
        .open(file_path, read_options())
        .map_err(|err| format!("无法读取文件「{name}」：{err}"))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("无法读取文件「{name}」：{err}"))?;
    if !metadata.is_file() {
        return Err(format!("「{name}」不是有效文件"));
    }
    let size = metadata.len();
    drop(file);
    // iOS 读取完元信息后结束安全作用域，真正传输时会重新打开
    close_source(app, raw);
    Ok(SourceFile {
        name,
        size,
        raw: raw.to_string(),
    })
}

/// 打开原始文件用于分片读取
pub fn open_source(app: &AppHandle, raw: &str) -> Result<std::fs::File, String> {
    let file_path = parse_path(raw)?;
    app.fs()
        .open(file_path, read_options())
        .map_err(|err| format!("无法打开文件：{err}"))
}

/// 结束 iOS 安全作用域资源的访问（其他平台为空操作）
pub fn close_source(app: &AppHandle, raw: &str) {
    #[cfg(target_os = "ios")]
    {
        if let Ok(file_path) = parse_path(raw) {
            let _ = app.fs().stop_accessing_security_scoped_resource(file_path);
        }
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = (app, raw);
    }
}

/// 解析文件名：普通路径直接取文件名，Android URI 通过 ContentResolver 查询
fn display_name(file_path: &FilePath, raw: &str) -> String {
    if let Some(path) = file_path.as_path() {
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            return name.to_string();
        }
    }
    if let Ok(path) = file_path.clone().into_path() {
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            return name.to_string();
        }
    }

    #[cfg(target_os = "android")]
    {
        if let Some(name) = android_uri::display_name(raw) {
            if !name.trim().is_empty() {
                return name;
            }
        }
    }

    // 取不到原始文件名时，退回 URI 末段，保证界面上仍可区分
    fallback_display_name(raw)
}

/// 从 URI 末段提取展示名。
/// Android content:// 与 iOS file:// 的路径可能包含 %E6%8A%A5 这类百分号编码，
/// 这里先解码再取最后一段，避免中文文件名显示成 URLEncode 形式。
fn fallback_display_name(raw: &str) -> String {
    let raw_path = raw.split('?').next().unwrap_or(raw);
    let decoded = percent_decode_str(raw_path).decode_utf8_lossy();
    let fallback = decoded
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("文件");
    fallback.replace(':', "_")
}

/// Android 上通过 ContentResolver 查询 content:// URI 的展示名称
#[cfg(target_os = "android")]
mod android_uri {
    use jni::objects::{JObject, JString, JValue};
    use jni::JavaVM;

    pub fn display_name(uri: &str) -> Option<String> {
        let context = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.ok()?;
        let mut env = vm.attach_current_thread().ok()?;
        let context = unsafe { JObject::from_raw(context.context().cast()) };
        let result = query(&mut env, &context, uri);
        // 清理可能遗留的 Java 异常，避免影响后续调用
        let _ = env.exception_clear();
        result
    }

    fn query(env: &mut jni::JNIEnv, context: &JObject, uri: &str) -> Option<String> {
        let resolver = env
            .call_method(
                context,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;

        let uri_string = env.new_string(uri).ok()?;
        let uri_object = JObject::from(uri_string);
        let parsed = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&uri_object)],
            )
            .ok()?
            .l()
            .ok()?;

        let null = JObject::null();
        let cursor = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&parsed),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                ],
            )
            .ok()?
            .l()
            .ok()?;
        if cursor.is_null() {
            return None;
        }

        let column = JObject::from(env.new_string("_display_name").ok()?);
        let index = env
            .call_method(
                &cursor,
                "getColumnIndex",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&column)],
            )
            .ok()?
            .i()
            .ok()?;

        let mut name = None;
        if index >= 0 {
            let value = env
                .call_method(
                    &cursor,
                    "getString",
                    "(I)Ljava/lang/String;",
                    &[JValue::Int(index)],
                )
                .ok()?
                .l()
                .ok()?;
            if !value.is_null() {
                if let Ok(text) = env.get_string(&JString::from(value)) {
                    let text: String = text.into();
                    if !text.trim().is_empty() {
                        name = Some(text);
                    }
                }
            }
        }

        let _ = env.call_method(&cursor, "close", "()V", &[]);
        name
    }
}

#[cfg(test)]
mod tests {
    use super::fallback_display_name;

    #[test]
    fn decodes_percent_encoded_content_uri_name() {
        assert_eq!(
            fallback_display_name(
                "content://com.android.providers.downloads.documents/document/primary%3ADownload%2F%E6%8A%A5%E5%91%8A.pdf"
            ),
            "报告.pdf"
        );
    }

    #[test]
    fn decodes_percent_encoded_file_uri_name() {
        assert_eq!(
            fallback_display_name("file:///tmp/%E6%B5%8B%E8%AF%95.txt"),
            "测试.txt"
        );
    }

    #[test]
    fn keeps_plain_name_unchanged() {
        assert_eq!(fallback_display_name("报告.pdf"), "报告.pdf");
    }
}
