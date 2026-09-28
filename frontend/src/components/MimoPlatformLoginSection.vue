<template>
  <div class="glass-card p-5">
    <div class="flex items-center justify-between mb-2">
      <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">MiMo 登录</span>
      <span
        class="inline-flex items-center gap-1 text-xs font-medium"
        :class="mimoDashboard.hasPlatformSession ? 'text-green-600 dark:text-green-400' : 'text-red-500'"
      >
        <span>{{ mimoDashboard.hasPlatformSession ? '✔' : '✖' }}</span>
        <span>{{ mimoDashboard.hasPlatformSession ? '已登录' : '未登录' }}</span>
      </span>
    </div>
    <p class="text-xs text-gray-500 dark:text-gray-400 mb-3">
      登录 MiMo Platform 获取详细用量数据
    </p>
    <div class="flex gap-2">
      <button
        @click="startLogin"
        class="px-4 py-2 bg-brand-500 hover:bg-brand-600 text-white text-xs font-semibold rounded-lg transition shadow-sm shadow-brand-500/20"
      >
        打开登录页面
      </button>
      <button
        v-if="mimoDashboard.hasPlatformSession"
        @click="logout"
        class="px-4 py-2 bg-red-500/90 hover:bg-red-600 text-white text-xs font-semibold rounded-lg transition shadow-sm shadow-red-500/20"
      >
        退出登录
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useMimoDashboardStore } from "@/stores/mimo-dashboard";
import { useSettingsStore } from "@/stores/mimo-settings";

const mimoDashboard = useMimoDashboardStore();
const settings = useSettingsStore();

onMounted(async () => {
  listen("mimo-login-complete", () => {
    mimoDashboard.hasPlatformSession = true;
  });
  listen("mimo-login-cancelled", () => {
    mimoDashboard.hasPlatformSession = false;
  });
  listen("mimo-trigger-refresh", (event: { payload: any }) => {
    if (event.payload && typeof event.payload === 'object' && 'has_platform_session' in event.payload) {
      mimoDashboard.applyData(event.payload);
    } else {
      mimoDashboard.refresh();
    }
  });
  mimoDashboard.hasPlatformSession = await invoke<boolean>("mimo_has_platform_session").catch(() => false);
});

async function startLogin() {
  await settings.openPlatformLogin();
}

async function logout() {
  mimoDashboard.hasPlatformSession = false;
  await settings.clearPlatformSession();
}
</script>
