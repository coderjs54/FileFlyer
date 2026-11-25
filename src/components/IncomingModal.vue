<script setup lang="ts">
import type { IncomingRequest } from "../types";
import { formatBytes } from "../lib/format";
import AppIcon from "./AppIcon.vue";

defineProps<{
  request: IncomingRequest;
}>();

const emit = defineEmits<{
  (event: "accept"): void;
  (event: "decline"): void;
}>();

/** 弹窗标题下方的文件摘要，避免在模板中构造伪 Transfer 对象 */
function requestSummary(request: IncomingRequest): string {
  if (!request.files.length) return "文件";
  if (request.files.length === 1) return request.files[0].name;
  return `${request.files[0].name} 等 ${request.files.length} 个文件`;
}
</script>

<template>
  <div class="modal-backdrop">
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="incoming-title">
      <div class="modal-head">
        <div class="modal-icon">
          <AppIcon name="arrow-down" :size="20" />
        </div>
        <div class="modal-head-text">
          <h2 id="incoming-title" class="modal-title">收到文件请求</h2>
          <p class="modal-sub">
            {{ request.peerName }} 想发送 {{ requestSummary(request) }}
          </p>
        </div>
      </div>

      <div class="incoming-files">
        <div v-for="(file, index) in request.files" :key="index" class="incoming-file">
          <span class="incoming-name" :title="file.name">{{ file.name }}</span>
          <span>{{ formatBytes(file.size) }}</span>
        </div>
      </div>

      <div class="incoming-total">
        <span>共 {{ request.files.length }} 个文件</span>
        <span>{{ formatBytes(request.totalBytes) }}</span>
      </div>

      <p class="modal-note">接收后会保存到本机目录，并自动进行 SHA-256 完整性校验。</p>

      <div class="modal-actions">
        <button class="btn btn-ghost" type="button" @click="emit('decline')">拒绝</button>
        <button class="btn btn-primary" type="button" @click="emit('accept')">接收</button>
      </div>
    </div>
  </div>
</template>
