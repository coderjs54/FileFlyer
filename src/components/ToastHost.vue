<script setup lang="ts">
import type { ToastMessage } from "../types";

defineProps<{
  toasts: ToastMessage[];
}>();
</script>

<template>
  <div class="toast-host" aria-live="polite">
    <div v-for="toast in toasts" :key="toast.id" class="toast" :class="toast.kind">
      {{ toast.text }}
    </div>
  </div>
</template>
