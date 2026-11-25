<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  IncomingRequest,
  Peer,
  Settings,
  SettingsPayload,
  ToastMessage,
  Transfer,
} from "./types";
import { api, errorText } from "./lib/api";
import { isActiveStatus, isTerminalStatus } from "./lib/format";
import AppIcon from "./components/AppIcon.vue";
import DeviceList from "./components/DeviceList.vue";
import TransferList from "./components/TransferList.vue";
import SettingsView from "./components/SettingsView.vue";
import IncomingModal from "./components/IncomingModal.vue";
import ToastHost from "./components/ToastHost.vue";

type Tab = "devices" | "transfers" | "settings";

const navItems: Array<{ key: Tab; label: string; icon: string }> = [
  { key: "devices", label: "附近设备", icon: "devices" },
  { key: "transfers", label: "传输记录", icon: "transfer" },
  { key: "settings", label: "设置", icon: "settings" },
];

const pageMetaMap: Record<Tab, { title: string; hint: string }> = {
  devices: {
    title: "附近设备",
    hint: "点击设备的「发送文件」，选择文件后即可开始传输。",
  },
  transfers: {
    title: "传输记录",
    hint: "所有文件都在本机与对方设备之间直连传输。",
  },
  settings: {
    title: "设置",
    hint: "调整设备名称、接收方式与文件保存位置。",
  },
};

const tab = ref<Tab>("devices");
const settings = ref<Settings | null>(null);
const peers = ref<Peer[]>([]);
const transfers = ref<Transfer[]>([]);
const incomingQueue = ref<IncomingRequest[]>([]);
const toasts = ref<ToastMessage[]>([]);

const unlisteners: UnlistenFn[] = [];
let toastSequence = 0;

const isMobile = computed(
  () =>
    settings.value?.platform === "android" ||
    settings.value?.platform === "ios",
);
const incoming = computed(() => incomingQueue.value[0] ?? null);
const activeCount = computed(
  () => transfers.value.filter((item) => isActiveStatus(item.status)).length,
);
const pageMeta = computed(() => pageMetaMap[tab.value]);

function sortTransfers(list: Transfer[]): Transfer[] {
  return [...list].sort((a, b) => b.createdAt - a.createdAt);
}

function pushToast(text: string, kind: ToastMessage["kind"] = "info") {
  const id = ++toastSequence;
  toasts.value.push({ id, text, kind });
  if (toasts.value.length > 4) {
    toasts.value.shift();
  }
  window.setTimeout(
    () => {
      toasts.value = toasts.value.filter((toast) => toast.id !== id);
    },
    kind === "error" ? 4200 : 2600,
  );
}

/** 从后端重新拉取完整状态，应用回到前台时也会调用 */
async function refresh() {
  const snapshot = await api.getSnapshot();
  settings.value = snapshot.settings;
  peers.value = snapshot.peers;
  transfers.value = sortTransfers(snapshot.transfers);

  const transferMap = new Map(snapshot.transfers.map((item) => [item.id, item]));
  incomingQueue.value = incomingQueue.value.filter((request) => {
    const transfer = transferMap.get(request.id);
    return !!transfer && transfer.status === "waiting";
  });
  // 移动端后台时可能错过事件，用快照补回等待确认的请求
  for (const transfer of snapshot.transfers) {
    if (transfer.direction !== "receive" || transfer.status !== "waiting") continue;
    if (incomingQueue.value.some((request) => request.id === transfer.id)) continue;
    incomingQueue.value.push({
      id: transfer.id,
      peerName: transfer.peerName,
      files: transfer.files.map((file) => ({
        name: file.name,
        size: file.size,
      })),
      totalBytes: transfer.totalBytes,
    });
  }
}

function handleTransferUpdate(transfer: Transfer) {
  const previous = transfers.value.find((item) => item.id === transfer.id);
  if (previous) {
    transfers.value = transfers.value.map((item) =>
      item.id === transfer.id ? transfer : item,
    );
  } else {
    transfers.value = sortTransfers([transfer, ...transfers.value]);
  }

  if (isTerminalStatus(transfer.status)) {
    incomingQueue.value = incomingQueue.value.filter(
      (request) => request.id !== transfer.id,
    );
  }

  if (!previous || previous.status !== transfer.status) {
    if (transfer.status === "failed" && transfer.error) {
      pushToast(transfer.error, "error");
    } else if (transfer.status === "declined") {
      pushToast(
        transfer.direction === "send" ? "对方拒绝了此次传输" : "已拒绝此次传输",
        "info",
      );
    } else if (transfer.status === "completed") {
      pushToast(
        transfer.direction === "receive"
          ? "文件接收完成，校验通过"
          : "文件发送完成，校验通过",
        "success",
      );
    }
  }
}

async function handleSend(peer: Peer) {
  try {
    const selected = await open({
      multiple: true,
      directory: false,
      title: `发送文件给「${peer.name}」`,
    });
    if (selected === null) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (!paths.length) return;

    await api.sendFiles(peer.id, paths);
    tab.value = "transfers";
    pushToast("已开始发送文件");
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function handlePing(ip: string) {
  try {
    await api.pingAddress(ip);
    pushToast("已向该地址发送连接请求，请稍候");
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function handleCancel(id: string) {
  try {
    await api.cancelTransfer(id);
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function handleClear() {
  try {
    transfers.value = await api.clearFinished();
    pushToast("已清除结束的记录");
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function handleOpenDir(path?: string | null) {
  try {
    await api.openDownloadDir(path ?? null);
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function handleSaveSettings(payload: SettingsPayload) {
  try {
    settings.value = await api.updateSettings(
      payload.deviceName,
      payload.autoAccept,
      payload.downloadDir,
    );
    pushToast("设置已保存", "success");
  } catch (error) {
    pushToast(errorText(error), "error");
  }
}

async function decideIncoming(accept: boolean) {
  const request = incoming.value;
  if (!request) return;
  try {
    await api.respondIncoming(request.id, accept);
  } catch (error) {
    pushToast(errorText(error), "error");
  }
  incomingQueue.value = incomingQueue.value.filter(
    (item) => item.id !== request.id,
  );
  if (accept) {
    tab.value = "transfers";
  }
}

function handleVisibility() {
  if (document.visibilityState === "visible") {
    refresh().catch((error) => pushToast(errorText(error), "error"));
  }
}

onMounted(async () => {
  try {
    unlisteners.push(
      await listen<Peer[]>("peers-changed", (event) => {
        peers.value = event.payload;
      }),
    );
    unlisteners.push(
      await listen<Transfer>("transfer-updated", (event) => {
        handleTransferUpdate(event.payload);
      }),
    );
    unlisteners.push(
      await listen<IncomingRequest>("incoming-request", (event) => {
        const request = event.payload;
        if (!incomingQueue.value.some((item) => item.id === request.id)) {
          incomingQueue.value.push(request);
        }
        pushToast(`收到来自 ${request.peerName} 的文件请求`);
      }),
    );
    unlisteners.push(
      await listen<string>("incoming-cancelled", (event) => {
        incomingQueue.value = incomingQueue.value.filter(
          (item) => item.id !== event.payload,
        );
        pushToast("对方已取消发送");
      }),
    );
    await refresh();
  } catch (error) {
    pushToast(errorText(error), "error");
  }
  document.addEventListener("visibilitychange", handleVisibility);
});

onUnmounted(() => {
  document.removeEventListener("visibilitychange", handleVisibility);
  for (const unlisten of unlisteners) {
    unlisten();
  }
});
</script>

<template>
  <div class="shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">
          <AppIcon name="transfer" :size="20" />
        </div>
        <div class="brand-text">
          <div class="brand-name">FileFlyer</div>
          <div class="brand-sub">局域网文件互传</div>
        </div>
      </div>

      <nav class="nav">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="nav-btn"
          :class="{ active: tab === item.key }"
          type="button"
          :aria-current="tab === item.key ? 'page' : undefined"
          @click="tab = item.key"
        >
          <AppIcon :name="item.icon" :size="18" />
          <span>{{ item.label }}</span>
          <span
            v-if="item.key === 'transfers' && activeCount > 0"
            class="nav-badge"
          >
            {{ activeCount }}
          </span>
        </button>
      </nav>

      <div class="sidebar-foot">
        <div class="self-card">
          <span class="self-dot"></span>
          <div class="self-meta">
            <div class="self-name">{{ settings?.deviceName ?? "正在启动…" }}</div>
            <div class="self-hint">在线 · 局域网直连</div>
          </div>
        </div>
      </div>
    </aside>

    <div class="main">
      <header class="mobile-top">
        <div class="brand-mark small">
          <AppIcon name="transfer" :size="17" />
        </div>
        <span class="mobile-title">FileFlyer</span>
        <span class="mobile-device">{{ settings?.deviceName ?? "" }}</span>
      </header>

      <main class="page">
        <div class="page-head">
          <div class="page-title-block">
            <h1>{{ pageMeta.title }}</h1>
            <p>{{ pageMeta.hint }}</p>
          </div>
          <div v-if="tab === 'devices'" class="discovery-chip">
            <span class="pulse-dot"></span>
            {{ peers.length ? `已发现 ${peers.length} 台设备` : "正在搜索设备" }}
          </div>
        </div>

        <template v-if="settings">
          <DeviceList
            v-if="tab === 'devices'"
            :peers="peers"
            :is-mobile="isMobile"
            @send="handleSend"
            @ping="handlePing"
          />
          <TransferList
            v-else-if="tab === 'transfers'"
            :transfers="transfers"
            :is-mobile="isMobile"
            @cancel="handleCancel"
            @open-dir="handleOpenDir"
            @clear="handleClear"
          />
          <SettingsView
            v-else-if="tab === 'settings'"
            :settings="settings"
            :is-mobile="isMobile"
            @save="handleSaveSettings"
            @error="(message: string) => pushToast(message, 'error')"
            @open-dir="handleOpenDir(settings.downloadDir)"
          />
        </template>
        <div v-else class="panel">
          <div class="empty">
            <p class="empty-title">正在读取设备信息…</p>
          </div>
        </div>
      </main>

      <nav class="bottom-nav">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="bottom-btn"
          :class="{ active: tab === item.key }"
          type="button"
          @click="tab = item.key"
        >
          <AppIcon :name="item.icon" :size="20" />
          <span>{{ item.label }}</span>
          <span
            v-if="item.key === 'transfers' && activeCount > 0"
            class="bottom-badge"
          >
            {{ activeCount }}
          </span>
        </button>
      </nav>
    </div>

    <IncomingModal
      v-if="incoming"
      :request="incoming"
      @accept="decideIncoming(true)"
      @decline="decideIncoming(false)"
    />
    <ToastHost :toasts="toasts" />
  </div>
</template>
