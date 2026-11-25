<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { Settings, SettingsPayload } from "../types";
import { platformLabel } from "../lib/format";
import AppIcon from "./AppIcon.vue";

const props = defineProps<{
  settings: Settings;
  isMobile: boolean;
}>();

const emit = defineEmits<{
  (event: "save", payload: SettingsPayload): void;
  (event: "error", message: string): void;
  (event: "open-dir"): void;
}>();

const nameDraft = ref(props.settings.deviceName);

watch(
  () => props.settings.deviceName,
  (name) => {
    nameDraft.value = name;
  },
);

const nameDirty = computed(
  () => nameDraft.value.trim() !== props.settings.deviceName,
);

function saveName() {
  const name = nameDraft.value.trim();
  if (!name) {
    emit("error", "设备名称不能为空");
    return;
  }
  if (!nameDirty.value) return;
  emit("save", {
    deviceName: name,
    autoAccept: props.settings.autoAccept,
    downloadDir: null,
  });
}

function toggleAutoAccept() {
  // 保留输入框中尚未保存的名称，避免切换开关时丢失编辑内容
  emit("save", {
    deviceName: nameDraft.value.trim() || props.settings.deviceName,
    autoAccept: !props.settings.autoAccept,
    downloadDir: null,
  });
}

/** 桌面端选择新的接收目录 */
async function chooseDownloadDir() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择接收文件的保存目录",
      defaultPath: props.settings.downloadDir || undefined,
    });
    if (typeof selected !== "string") return;
    if (selected === props.settings.downloadDir) return;
    emit("save", {
      deviceName: nameDraft.value.trim() || props.settings.deviceName,
      autoAccept: props.settings.autoAccept,
      downloadDir: selected,
    });
  } catch (error) {
    emit("error", error instanceof Error ? error.message : String(error));
  }
}
</script>

<template>
  <div class="settings">
    <section>
      <div class="settings-group-title">本机</div>
      <div class="settings-group">
        <div class="setting-row row-input">
          <div class="setting-info">
            <div class="setting-label">设备名称</div>
            <div class="setting-desc">附近设备列表中展示的名称</div>
          </div>
          <div class="setting-control">
            <input
              v-model="nameDraft"
              class="text-input"
              type="text"
              maxlength="40"
              @keyup.enter="saveName"
            />
            <button
              class="btn btn-primary btn-sm"
              type="button"
              :disabled="!nameDirty"
              @click="saveName"
            >
              保存
            </button>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-label">当前平台</div>
          </div>
          <div class="setting-control">
            <span class="setting-value">{{ platformLabel(settings.platform) }}</span>
          </div>
        </div>
      </div>
    </section>

    <section>
      <div class="settings-group-title">接收</div>
      <div class="settings-group">
        <div class="setting-row row-switch">
          <div class="setting-info">
            <div class="setting-label">自动接收文件</div>
            <div class="setting-desc">
              开启后同一局域网内的设备可以直接发送，不再弹出确认窗口
            </div>
          </div>
          <div class="setting-control">
            <button
              class="switch"
              :class="{ on: settings.autoAccept }"
              type="button"
              role="switch"
              :aria-checked="settings.autoAccept"
              @click="toggleAutoAccept"
            >
              <span class="sr-only">自动接收文件</span>
            </button>
          </div>
        </div>

        <div class="setting-row row-dir">
          <div class="setting-info">
            <div class="setting-label">保存位置</div>
            <div class="setting-desc">接收完成的文件会保存在这里</div>
          </div>
          <div class="setting-control dir-control">
            <span class="path-text" :title="settings.downloadDir">
              {{ settings.downloadDir }}
            </span>
            <button
              v-if="!isMobile"
              class="btn btn-ghost btn-sm"
              type="button"
              @click="chooseDownloadDir"
            >
              更改
            </button>
            <button
              v-if="!isMobile"
              class="btn btn-ghost btn-sm"
              type="button"
              @click="emit('open-dir')"
            >
              <AppIcon name="folder" :size="14" />
              显示
            </button>
          </div>
        </div>

        <div v-if="isMobile" class="setting-row">
          <p class="mobile-note">
            移动端受系统存储权限限制，文件保存在应用目录中；传输完成后可在记录里查看保存位置。
          </p>
        </div>
      </div>
    </section>

    <section>
      <div class="settings-group-title">关于</div>
      <div class="settings-group">
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-label">传输方式</div>
            <div class="setting-desc">
              设备之间通过局域网直连传输，不经过任何服务器
            </div>
          </div>
          <div class="setting-control">
            <span class="setting-value">局域网直连</span>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-label">完整性校验</div>
            <div class="setting-desc">每个文件接收完成后比对 SHA-256</div>
          </div>
          <div class="setting-control">
            <span class="setting-value">SHA-256</span>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-label">版本</div>
          </div>
          <div class="setting-control">
            <span class="setting-value">v{{ settings.appVersion }}</span>
          </div>
        </div>
      </div>
      <p class="settings-footnote">无需联网，文件不会上传到云端。</p>
    </section>
  </div>
</template>
