import { invoke } from "@tauri-apps/api/core";
import type { Settings, Snapshot, Transfer } from "../types";

/** Rust 命令的前端封装 */
export const api = {
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),

  sendFiles: (peerId: string, paths: string[]) =>
    invoke<string>("send_files", { peerId, paths }),

  respondIncoming: (id: string, accept: boolean) =>
    invoke<void>("respond_incoming", { id, accept }),

  cancelTransfer: (id: string) => invoke<void>("cancel_transfer", { id }),

  clearFinished: () => invoke<Transfer[]>("clear_finished"),

  updateSettings: (
    deviceName: string,
    autoAccept: boolean,
    downloadDir: string | null,
  ) => invoke<Settings>("update_settings", { deviceName, autoAccept, downloadDir }),

  pingAddress: (ip: string) => invoke<void>("ping_address", { ip }),

  openDownloadDir: (path?: string | null) =>
    invoke<void>("open_download_dir", { path: path ?? null }),
};

/** 把后端或系统抛出的异常统一成可展示的文案 */
export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "操作失败，请稍后重试";
}
