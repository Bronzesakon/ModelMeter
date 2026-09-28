<template>
  <div class="glass-card p-5">
    <div class="flex items-center justify-between mb-2">
      <span class="text-sm font-semibold text-gray-700 dark:text-gray-300">DeepSeek Platform 登录</span>
      <span
        class="inline-flex items-center gap-1 text-xs font-medium"
        :class="dashboard.hasPlatformSession ? 'text-green-600 dark:text-green-400' : 'text-red-500'"
      >
        <span>{{ dashboard.hasPlatformSession ? '✔' : '✖' }}</span>
        <span>{{ dashboard.hasPlatformSession ? '已登录' : '未登录' }}</span>
      </span>
    </div>
    <p class="text-xs text-gray-500 dark:text-gray-400 mb-3">
      登录 DeepSeek Platform 获取详细用量数据
    </p>
    <div class="flex gap-2">
      <button
        @click="startLogin"
        class="px-4 py-2 bg-brand-500 hover:bg-brand-600 text-white text-xs font-semibold rounded-lg transition shadow-sm shadow-brand-500/20"
      >
        打开登录页面
      </button>
      <button
        v-if="dashboard.hasPlatformSession"
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
import { useDashboardStore } from "@/stores/dashboard";
import { useSettingsStore } from "@/stores/settings";

const dashboard = useDashboardStore();
const settings = useSettingsStore();

onMounted(async () => {
  listen("ds-login-complete", () => {
    dashboard.hasPlatformSession = true;
  });
  listen("ds-login-cancelled", () => {
    dashboard.hasPlatformSession = false;
  });
  listen("ds-trigger-refresh", (event: { payload: any }) => {
    if (event.payload && typeof event.payload === 'object' && 'has_platform_session' in event.payload) {
      dashboard.applyData(event.payload);
    } else {
      dashboard.refresh();
    }
  });
  dashboard.hasPlatformSession = await invoke<boolean>("ds_has_platform_session").catch(() => false);
});

async function startLogin() {
  await settings.openPlatformLogin();
}

async function logout() {
  dashboard.hasPlatformSession = false;
  await settings.clearPlatformSession();
}
</script>
