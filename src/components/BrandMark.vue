<script setup lang="ts">
defineProps<{
  size?: number
}>()
</script>

<template>
  <svg
    :width="size ?? 34"
    :height="size ?? 34"
    viewBox="0 0 40 40"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="pm3-mark-a" x1="5" y1="4" x2="35" y2="36" gradientUnits="userSpaceOnUse">
        <stop stop-color="var(--accent-strong)" />
        <stop offset="0.55" stop-color="var(--accent)" />
        <stop offset="1" stop-color="var(--secondary)" />
      </linearGradient>
      <linearGradient id="pm3-mark-b" x1="10" y1="10" x2="30" y2="31" gradientUnits="userSpaceOnUse">
        <stop stop-color="#ffffff" stop-opacity="0.95" />
        <stop offset="1" stop-color="#ffffff" stop-opacity="0.55" />
      </linearGradient>
    </defs>
    <rect x="1" y="1" width="38" height="38" rx="11" fill="url(#pm3-mark-a)" fill-opacity="0.14" />
    <rect x="1" y="1" width="38" height="38" rx="11" stroke="url(#pm3-mark-a)" stroke-opacity="0.48" />
    <path
      d="M11.5 12.2h9.2c4.2 0 7.1 2.6 7.1 6.4 0 3.9-2.9 6.5-7.1 6.5h-4.3v4.2h-4.9V12.2Zm4.9 3.9v5.2h4c1.8 0 2.9-1 2.9-2.7 0-1.6-1.1-2.5-2.9-2.5h-4Z"
      fill="url(#pm3-mark-b)"
    />
    <path
      d="M14 7.2h12.2M14 32.8h12.2"
      stroke="url(#pm3-mark-a)"
      stroke-width="1.4"
      stroke-linecap="round"
      stroke-opacity="0.5"
    />
    <circle cx="31.4" cy="7.6" r="2.2" fill="var(--accent)" />
    <circle cx="31.4" cy="7.6" r="4.4" stroke="var(--accent)" stroke-opacity="0.25" />
  </svg>
</template>
