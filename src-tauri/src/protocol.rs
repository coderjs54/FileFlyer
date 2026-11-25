use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 单个数据分片大小：256 KiB，兼顾吞吐与移动端内存占用
pub const CHUNK_SIZE: usize = 256 * 1024;
/// 控制帧上限，避免异常数据导致过大内存分配
const MAX_CONTROL_SIZE: usize = 2 * 1024 * 1024;
/// 单帧硬上限
const MAX_FRAME_SIZE: usize = 8 * 1024 * 1024;
const FRAME_CONTROL: u8 = 1;
const FRAME_CHUNK: u8 = 2;

/// 文件元信息（仅包含文件名与大小，不携带本地路径）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub name: String,
    pub size: u64,
}

/// 传输控制消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Control {
    /// 发送方发起传输请求
    Offer {
        id: String,
        from_id: String,
        from_name: String,
        files: Vec<FileMeta>,
    },
    /// 接收方同意接收
    Accept,
    /// 接收方拒绝接收
    Reject { reason: Option<String> },
    /// 任意一方取消传输
    Cancel { reason: Option<String> },
    /// 单个文件发送完毕，附带发送方计算的 SHA-256
    FileComplete { index: usize, sha256: String },
    /// 接收方返回校验结果
    FileVerified { index: usize, ok: bool },
    /// 发送方通知全部文件发送完成
    Finish { ok: bool, error: Option<String> },
    /// 接收方返回最终结果
    Result { ok: bool, error: Option<String> },
}

/// 解包后的帧
pub enum Frame {
    Control(Control),
    Chunk { index: u32, data: Vec<u8> },
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// 写入一个 JSON 控制帧
pub async fn write_control<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    control: &Control,
) -> io::Result<()> {
    let payload =
        serde_json::to_vec(control).map_err(|err| invalid_data(format!("编码控制消息失败：{err}")))?;
    write_frame(writer, FRAME_CONTROL, &payload).await
}

/// 写入一个文件分片帧
pub async fn write_chunk<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    index: u32,
    data: &[u8],
) -> io::Result<()> {
    let mut payload = Vec::with_capacity(4 + data.len());
    payload.extend_from_slice(&index.to_be_bytes());
    payload.extend_from_slice(data);
    write_frame(writer, FRAME_CHUNK, &payload).await
}

async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    kind: u8,
    payload: &[u8],
) -> io::Result<()> {
    if payload.len() > MAX_FRAME_SIZE {
        return Err(invalid_data("待发送的数据帧超过大小限制"));
    }
    let mut header = [0u8; 5];
    header[0] = kind;
    header[1..].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    writer.write_all(&header).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

/// 读取一个完整帧（长度前缀 + 帧体）
pub async fn read_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> io::Result<Frame> {
    let mut header = [0u8; 5];
    reader.read_exact(&mut header).await?;
    let kind = header[0];
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    // 按帧类型限制最大长度：数据帧不应超过约定分片大小
    let limit = match kind {
        FRAME_CHUNK => CHUNK_SIZE + 4,
        FRAME_CONTROL => MAX_CONTROL_SIZE,
        _ => 0,
    };
    if len > limit {
        return Err(invalid_data("收到的数据帧超过大小限制"));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    match kind {
        FRAME_CONTROL => {
            let control = serde_json::from_slice(&payload)
                .map_err(|err| invalid_data(format!("无法解析控制消息：{err}")))?;
            Ok(Frame::Control(control))
        }
        FRAME_CHUNK => {
            if payload.len() < 4 {
                return Err(invalid_data("文件分片数据不完整"));
            }
            let index = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
            Ok(Frame::Chunk {
                index,
                data: payload[4..].to_vec(),
            })
        }
        other => Err(invalid_data(format!("未知的帧类型：{other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn control_frame_roundtrip() {
        let (mut client, mut server) = tokio::io::duplex(64 * 1024);
        let offer = Control::Offer {
            id: "transfer-1".into(),
            from_id: "device-a".into(),
            from_name: "电脑".into(),
            files: vec![FileMeta {
                name: "报告.pdf".into(),
                size: 1024,
            }],
        };
        write_control(&mut client, &offer).await.unwrap();
        match read_frame(&mut server).await.unwrap() {
            Frame::Control(Control::Offer { id, files, .. }) => {
                assert_eq!(id, "transfer-1");
                assert_eq!(files[0].name, "报告.pdf");
            }
            _ => panic!("应收到 Offer 控制帧"),
        }
    }

    #[tokio::test]
    async fn chunk_frame_roundtrip() {
        let (mut client, mut server) = tokio::io::duplex(64 * 1024);
        let data = vec![7u8; 4096];
        write_chunk(&mut client, 3, &data).await.unwrap();
        match read_frame(&mut server).await.unwrap() {
            Frame::Chunk { index, data: got } => {
                assert_eq!(index, 3);
                assert_eq!(got, data);
            }
            _ => panic!("应收到数据分片帧"),
        }
    }
}
