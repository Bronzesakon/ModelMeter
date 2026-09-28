<template>
  <div ref="rootEl" class="p-4 glass-root settings-surface">
    <div data-tauri-drag-region class="title-bar mb-2">
      <div class="flex items-center gap-1.5">
        <img src="/icon.png" class="w-3.5 h-3.5 dark:brightness-150" />
        <span class="text-blue-600 dark:text-blue-400 font-semibold text-sm tracking-wide">设置</span>
      </div>
      <div class="title-actions">
        <button aria-label="关闭" class="title-btn close-btn" @click="closeWindow"><CloseIcon /></button>
      </div>
    </div>
    <div class="space-y-3">
      <PlatformLoginSection />
      <MimoPlatformLoginSection />
      <div class="glass-card p-5">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-300 block mb-3">MiMo 付费模式</span>
        <div class="grid grid-cols-2 gap-2">
          <button
            @click="mimoSettings.setPaymentMode('token')" :disabled="mimoSettings.modeSwitching"
            class="py-2 text-xs font-medium rounded-lg transition disabled:opacity-50 disabled:cursor-not-allowed"
            :class="mimoSettings.paymentMode === 'token' ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/30' : 'bg-white/60 dark:bg-zinc-600/60 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm hover:bg-white/80 dark:hover:bg-zinc-500/70'">Token</button>
          <button
            @click="mimoSettings.setPaymentMode('plan')" :disabled="mimoSettings.modeSwitching"
            class="py-2 text-xs font-medium rounded-lg transition disabled:opacity-50 disabled:cursor-not-allowed"
            :class="mimoSettings.paymentMode === 'plan' ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/30' : 'bg-white/60 dark:bg-zinc-600/60 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm hover:bg-white/80 dark:hover:bg-zinc-500/70'">套餐</button>
        </div>
      </div>
      <div class="glass-card p-5">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-300 block mb-3">主题</span>
        <div class="grid grid-cols-3 gap-2">
          <button @click="theme.setMode('light')"
            class="py-2 text-xs font-medium rounded-lg transition"
            :class="theme.mode === 'light' ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/30' : 'bg-white/60 dark:bg-zinc-600/60 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm hover:bg-white/80 dark:hover:bg-zinc-500/70'">浅色</button>
          <button @click="theme.setMode('dark')"
            class="py-2 text-xs font-medium rounded-lg transition"
            :class="theme.mode === 'dark' ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/30' : 'bg-white/60 dark:bg-zinc-600/60 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm hover:bg-white/80 dark:hover:bg-zinc-500/70'">深色</button>
          <button @click="theme.setMode('system')"
            class="py-2 text-xs font-medium rounded-lg transition"
            :class="theme.mode === 'system' ? 'bg-brand-500 text-white shadow-sm shadow-brand-500/30' : 'bg-white/60 dark:bg-zinc-600/60 text-gray-800 dark:text-gray-200 border border-gray-400/40 dark:border-white/20 shadow-sm hover:bg-white/80 dark:hover:bg-zinc-500/70'">跟随系统</button>
        </div>
      </div>
      <RefreshIntervalPicker />
      <div class="glass-card p-5">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-300 block mb-3">贴边隐藏</span>
        <div class="flex items-center justify-between">
          <span class="text-xs text-gray-500 dark:text-gray-400">窗口贴到屏幕顶部时自动隐藏内容</span>
          <button @click="settings.setEdgeSnapEnabled(!settings.edgeSnapEnabled)"
            class="relative w-10 h-5 rounded-full transition-colors duration-200 flex-shrink-0"
            :class="settings.edgeSnapEnabled ? 'bg-brand-500' : 'bg-gray-300 dark:bg-gray-600'">
            <span class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-200"
              :class="settings.edgeSnapEnabled ? 'translate-x-[20px]' : 'translate-x-0'"></span>
          </button>
        </div>
      </div>
      <div class="glass-card p-5">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-300 block mb-3">开机启动</span>
        <div class="flex items-center justify-between">
          <span class="text-xs text-gray-500 dark:text-gray-400">电脑开机时自动运行此程序</span>
          <button @click="toggleAutoStart"
            class="relative w-10 h-5 rounded-full transition-colors duration-200 flex-shrink-0 cursor-pointer"
            :class="[settings.autoStart ? 'bg-brand-500' : 'bg-gray-300 dark:bg-gray-600']">
            <span class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-200"
              :class="settings.autoStart ? 'translate-x-[20px]' : 'translate-x-0'"></span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, nextTick } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { invoke } from "@tauri-apps/api/core";
import { useThemeStore } from "@/stores/theme";
import { useSettingsStore } from "@/stores/settings";
import { useSettingsStore as useMimoSettingsStore } from "@/stores/mimo-settings";
import PlatformLoginSection from "@/components/PlatformLoginSection.vue";
import MimoPlatformLoginSection from "@/components/MimoPlatformLoginSection.vue";
import RefreshIntervalPicker from "@/components/RefreshIntervalPicker.vue";
import CloseIcon from "@/components/icons/CloseIcon.vue";

const theme = useThemeStore();
const settings = useSettingsStore();
const mimoSettings = useMimoSettingsStore();
const rootEl = ref<HTMLElement>();

onMounted(async () => {
  settings.loadEdgeSnapEnabled();
  settings.loadRefreshInterval();
  settings.loadAutoStart();
  mimoSettings.loadPaymentMode();

  await nextTick();
  if (rootEl.value) {
    const resize = () => {
      const h = rootEl.value!.scrollHeight;
      if (h > 0) getCurrentWindow().setSize(new LogicalSize(400, h)).then(() => {
        invoke("recenter_settings");
      });
    };
    resize();
    new ResizeObserver(() => resize()).observe(rootEl.value);
  }
});

function closeWindow() { invoke("hide_settings_window"); }
function toggleAutoStart() { settings.setAutoStart(!settings.autoStart); }
</script>
