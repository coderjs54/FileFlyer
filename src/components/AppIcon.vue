<script setup lang="ts">
withDefaults(
  defineProps<{
    name: string;
    size?: number;
  }>(),
  { size: 20 },
);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'devices'">
      <rect x="2.5" y="4" width="13" height="10" rx="2" />
      <path d="M6 18h6" />
      <rect x="16.5" y="9" width="5" height="11" rx="1.5" />
      <path d="M19 17h.01" />
    </template>
    <template v-else-if="name === 'transfer'">
      <path d="M4 8.5h13" />
      <path d="m13.5 4.5 4 4-4 4" />
      <path d="M20 15.5H7" />
      <path d="m10.5 11.5-4 4 4 4" />
    </template>
    <template v-else-if="name === 'settings'">
      <circle cx="12" cy="12" r="3" />
      <path
        d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.1A1.7 1.7 0 0 0 9 19.33a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.64 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.1A1.7 1.7 0 0 0 4.67 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.64 1.7 1.7 0 0 0 10.03 3.1V3a2 2 0 1 1 4 0v.1A1.7 1.7 0 0 0 15 4.67a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.36 9v.03A1.7 1.7 0 0 0 20.9 10.03H21a2 2 0 1 1 0 4h-.1A1.7 1.7 0 0 0 19.4 15Z"
      />
    </template>
    <template v-else-if="name === 'send'">
      <path d="m22 2-7 20-4-9-9-4Z" />
      <path d="M22 2 11 13" />
    </template>
    <template v-else-if="name === 'arrow-up'">
      <path d="M12 19V5" />
      <path d="m5 12 7-7 7 7" />
    </template>
    <template v-else-if="name === 'arrow-down'">
      <path d="M12 5v14" />
      <path d="m19 12-7 7-7-7" />
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h3l2 2h8A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
    </template>
    <template v-else-if="name === 'shield'">
      <path d="M12 3.5 19 6v5.5c0 4.4-2.9 7.8-7 9.5-4.1-1.7-7-5.1-7-9.5V6z" />
      <path d="m9 12 2 2 4-4" />
    </template>
    <template v-else-if="name === 'radar'">
      <circle cx="12" cy="12" r="2" />
      <path d="M12 3a9 9 0 0 1 9 9" />
      <path d="M12 7a5 5 0 0 1 5 5" />
      <path d="M12 21a9 9 0 0 1-9-9" />
      <path d="M12 17a5 5 0 0 1-5-5" />
    </template>
    <template v-else-if="name === 'phone'">
      <rect x="6.5" y="2.5" width="11" height="19" rx="2.5" />
      <path d="M10.5 18.5h3" />
    </template>
    <template v-else-if="name === 'laptop'">
      <rect x="3.5" y="4.5" width="17" height="11" rx="2" />
      <path d="M2 19.5h20" />
    </template>
    <template v-else-if="name === 'check'">
      <path d="m20 6-11 11-5-5" />
    </template>
    <template v-else-if="name === 'close'">
      <path d="M18 6 6 18" />
      <path d="m6 6 12 12" />
    </template>
    <template v-else-if="name === 'alert'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7.5v5" />
      <path d="M12 16.5h.01" />
    </template>
    <template v-else-if="name === 'link'">
      <path d="M10 13a5 5 0 0 0 7.54.54l2-2a5 5 0 0 0-7.07-7.07l-1.14 1.14" />
      <path d="M14 11a5 5 0 0 0-7.54-.54l-2 2a5 5 0 0 0 7.07 7.07l1.14-1.14" />
    </template>
    <template v-else-if="name === 'wifi'">
      <path d="M5 12.55a11 11 0 0 1 14 0" />
      <path d="M8.5 16.05a6 6 0 0 1 7 0" />
      <path d="M12 20h.01" />
      <path d="M2 8.82a15 15 0 0 1 20 0" />
    </template>
  </svg>
</template>
