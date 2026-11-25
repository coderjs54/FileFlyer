/** 局域网内发现的设备 */
export interface Peer {
  id: string;
  name: string;
  platform: string;
  ip: string;
  port: number;
}

/** 一次传输中的单个文件 */
export interface TransferFile {
  name: string;
  size: number;
  transferred: number;
  status: string;
}

export type TransferDirection = "send" | "receive";

export type TransferStatus =
  | "preparing"
  | "connecting"
  | "waiting"
  | "transferring"
  | "verifying"
  | "completed"
  | "failed"
  | "cancelled"
  | "declined";

/** 一次传输任务 */
export interface Transfer {
  id: string;
  direction: TransferDirection;
  peerId: string;
  peerName: string;
  files: TransferFile[];
  totalBytes: number;
  transferredBytes: number;
  status: TransferStatus;
  speed: number;
  error: string | null;
  createdAt: number;
  dir: string | null;
}

export interface IncomingFile {
  name: string;
  size: number;
}

/** 接收端收到的传输请求 */
export interface IncomingRequest {
  id: string;
  peerName: string;
  files: IncomingFile[];
  totalBytes: number;
}

export interface Settings {
  deviceId: string;
  deviceName: string;
  platform: string;
  autoAccept: boolean;
  downloadDir: string;
  tcpPort: number;
  appVersion: string;
}

export interface Snapshot {
  settings: Settings;
  peers: Peer[];
  transfers: Transfer[];
}

export type ToastKind = "info" | "error" | "success";

export interface ToastMessage {
  id: number;
  text: string;
  kind: ToastKind;
}

/** 设置页提交给后端的变更 */
export interface SettingsPayload {
  deviceName: string;
  autoAccept: boolean;
  downloadDir: string | null;
}
