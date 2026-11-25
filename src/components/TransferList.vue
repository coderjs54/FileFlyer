<script setup lang="ts">
import { computed } from "vue";
import type { Transfer } from "../types";
import {
  directionLabel,
  fileSummary,
  formatBytes,
  formatSpeed,
  isActiveStatus,
  isTerminalStatus,
  percentOf,
  relativeTime,
  statusLabel,
} from "../lib/format";
import AppIcon from "./AppIcon.vue";

const props = defineProps<{
  transfers: Transfer[];
  isMobile: boolean;
}>();

const emit = defineEmits<{
  (event: "cancel", id: string): void;
  (event: "open-dir", path: string): void;
  (event: "clear"): void;
}>();

const hasFinished = computed(() =>
  props.transfers.some((transfer) => isTerminalStatus(transfer.status)),
);

function progressClass(transfer: Transfer): string {
  if (transfer.status === "failed") return "danger";
  if (transfer.status === "completed") return "success";
  if (transfer.status === "cancelled" || transfer.status === "declined") {
    return "muted";
  }
  return "";
}

function canOpenFolder(transfer: Transfer): boolean {
  return (
    !props.isMobile &&
    transfer.direction === "receive" &&
    transfer.status === "completed" &&
    !!transfer.dir
  );
}
</script>

<template>
  <div>
    <div v-if="transfers.length" class="list-toolbar">
      <span>共 {{ transfers.length }} 条记录</span>
      <button v-if="hasFinished" class="link-btn" type="button" @click="emit('clear')">
        清除已结束
      </button>
    </div>

    <div v-if="transfers.length" class="transfer-list">
      <div v-for="transfer in transfers" :key="transfer.id" class="transfer-row">
        <div class="transfer-dir" :class="transfer.direction">
          <AppIcon
            :name="transfer.direction === 'send' ? 'arrow-up' : 'arrow-down'"
            :size="18"
          />
        </div>

        <div class="transfer-main">
          <div class="transfer-top">
            <span class="transfer-title">{{ directionLabel(transfer) }}</span>
            <span
              class="transfer-status"
              :class="{
                'is-failed': transfer.status === 'failed',
                'is-success': transfer.status === 'completed',
              }"
            >
              {{ statusLabel(transfer) }}
              <template v-if="transfer.status === 'transferring' && transfer.speed">
                · {{ formatSpeed(transfer.speed) }}
              </template>
            </span>
          </div>

          <div class="transfer-files">{{ fileSummary(transfer) }}</div>

          <div class="progress-track">
            <div
              class="progress-fill"
              :class="progressClass(transfer)"
              :style="{ width: percentOf(transfer) + '%' }"
            ></div>
          </div>

          <div class="transfer-bottom">
            <span>
              {{ relativeTime(transfer.createdAt) }} ·
              {{ formatBytes(transfer.transferredBytes) }} /
              {{ formatBytes(transfer.totalBytes) }}
            </span>
            <span class="transfer-actions">
              <button
                v-if="isActiveStatus(transfer.status)"
                class="btn btn-ghost btn-sm"
                type="button"
                @click="emit('cancel', transfer.id)"
              >
                取消
              </button>
              <button
                v-if="canOpenFolder(transfer)"
                class="btn btn-ghost btn-sm"
                type="button"
                @click="emit('open-dir', transfer.dir as string)"
              >
                <AppIcon name="folder" :size="14" />
                在文件夹中显示
              </button>
              <span v-if="transfer.status === 'completed'" class="verified">
                <AppIcon name="shield" :size="14" />
                校验通过
              </span>
            </span>
          </div>

          <p v-if="transfer.error" class="transfer-error">{{ transfer.error }}</p>
        </div>
      </div>
    </div>

    <div v-else class="panel">
      <div class="empty">
        <div class="empty-icon">
          <AppIcon name="transfer" :size="26" />
        </div>
        <p class="empty-title">暂无传输记录</p>
        <p class="empty-hint">
          在“附近设备”中选择一台设备并发送文件，进度会显示在这里
        </p>
      </div>
    </div>
  </div>
</template>
