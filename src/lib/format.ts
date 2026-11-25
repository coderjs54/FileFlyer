import type { Transfer } from "../types";

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** 文件大小格式化，如 1.2 MB */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const index = Math.min(
    UNITS.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  const value = bytes / Math.pow(1024, index);
  const digits = index === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[index]}`;
}

/** 传输速度格式化 */
export function formatSpeed(bytesPerSecond: number): string {
  if (!bytesPerSecond || bytesPerSecond <= 0) return "";
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** 传输进度百分比 */
export function percentOf(transfer: Transfer): number {
  if (transfer.status === "completed") return 100;
  if (!transfer.totalBytes) return 0;
  const percent = Math.round(
    (transfer.transferredBytes / transfer.totalBytes) * 100,
  );
  return Math.max(0, Math.min(100, percent));
}

/** 传输状态的中文描述 */
export function statusLabel(transfer: Transfer): string {
  switch (transfer.status) {
    case "preparing":
      return "准备中";
    case "connecting":
      return "正在连接";
    case "waiting":
      return transfer.direction === "send" ? "等待对方确认" : "等待确认";
    case "transferring":
      return "传输中";
    case "verifying":
      return "正在校验";
    case "completed":
      return "已完成";
    case "failed":
      return "传输失败";
    case "cancelled":
      return "已取消";
    case "declined":
      return transfer.direction === "send" ? "对方已拒绝" : "已拒绝";
    default:
      return transfer.status;
  }
}

/** 传输方向描述 */
export function directionLabel(transfer: Transfer): string {
  return transfer.direction === "send"
    ? `发送给 ${transfer.peerName}`
    : `来自 ${transfer.peerName}`;
}

/** 文件列表摘要 */
export function fileSummary(transfer: Transfer): string {
  if (transfer.files.length === 0) return "无文件";
  if (transfer.files.length === 1) return transfer.files[0].name;
  return `${transfer.files[0].name} 等 ${transfer.files.length} 个文件`;
}

const TERMINAL_STATUSES = new Set(["completed", "failed", "cancelled", "declined"]);

export function isTerminalStatus(status: string): boolean {
  return TERMINAL_STATUSES.has(status);
}

/** 是否还有未完成的操作可取消 */
export function isActiveStatus(status: string): boolean {
  return !isTerminalStatus(status);
}

/** 时间戳转相对时间 */
export function relativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60 * 1000) return "刚刚";
  if (diff < 60 * 60 * 1000) return `${Math.floor(diff / 60000)} 分钟前`;
  if (diff < 24 * 60 * 60 * 1000) return `${Math.floor(diff / 3600000)} 小时前`;
  const date = new Date(timestamp);
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}

/** 平台名称展示 */
export function platformLabel(platform: string): string {
  switch (platform) {
    case "windows":
      return "Windows";
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    case "android":
      return "Android";
    case "ios":
      return "iOS";
    default:
      return platform;
  }
}
