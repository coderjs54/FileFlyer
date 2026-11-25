<script setup lang="ts">
import { ref } from "vue";
import type { Peer } from "../types";
import { platformLabel } from "../lib/format";
import AppIcon from "./AppIcon.vue";

defineProps<{
  peers: Peer[];
  isMobile: boolean;
}>();

const emit = defineEmits<{
  (event: "send", peer: Peer): void;
  (event: "ping", ip: string): void;
}>();

const showManual = ref(false);
const ip = ref("");
const inputError = ref("");

function platformIcon(platform: string): string {
  return platform === "android" || platform === "ios" ? "phone" : "laptop";
}

function submitManual() {
  const value = ip.value.trim();
  const parts = value.split(".");
  const valid =
    parts.length === 4 &&
    parts.every(
      (part) =>
        /^\d{1,3}$/.test(part) &&
        Number(part) >= 0 &&
        Number(part) <= 255,
    );
  if (!valid) {
    inputError.value = "请输入正确的 IPv4 地址，例如 192.168.1.10";
    return;
  }
  inputError.value = "";
  emit("ping", value);
}
</script>

<template>
  <div>
    <div class="panel">
      <div v-if="peers.length" class="device-list">
        <div v-for="peer in peers" :key="peer.id" class="device-row">
          <div class="device-icon">
            <AppIcon :name="platformIcon(peer.platform)" :size="22" />
          </div>
          <div class="device-meta">
            <div class="device-name">{{ peer.name }}</div>
            <div class="device-sub">
              {{ platformLabel(peer.platform) }} · {{ peer.ip }}
            </div>
          </div>
          <button class="btn btn-primary" @click="emit('send', peer)">
            <AppIcon name="send" :size="15" />
            发送文件
          </button>
        </div>
      </div>

      <div v-else class="empty">
        <div class="empty-icon">
          <AppIcon name="radar" :size="26" />
        </div>
        <p class="empty-title">正在搜索附近设备</p>
        <p class="empty-hint">
          {{
            isMobile
              ? "请确保两台设备连接同一个 Wi-Fi，并保持本应用在前台运行"
              : "请确保两台设备处于同一局域网，并在防火墙提示时允许访问"
          }}
        </p>
      </div>
    </div>

    <div class="manual">
      <button v-if="!showManual" class="link-btn" type="button" @click="showManual = true">
        找不到设备？通过 IP 地址连接
      </button>
      <form v-else class="manual-form" @submit.prevent="submitManual">
        <input
          v-model="ip"
          class="text-input"
          type="text"
          inputmode="text"
          autocomplete="off"
          placeholder="对方设备的 IP，例如 192.168.1.10"
          @input="inputError = ''"
        />
        <button class="btn btn-ghost" type="submit">连接</button>
        <button class="link-btn" type="button" @click="showManual = false; inputError = ''">
          取消
        </button>
      </form>
      <p v-if="inputError" class="form-error">{{ inputError }}</p>
    </div>
  </div>
</template>
