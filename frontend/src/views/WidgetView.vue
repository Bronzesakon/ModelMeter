<template>
  <div ref="rootEl" :class="[collapsed ? 'px-5 pt-2 glass-root' : 'p-4 glass-root', { 'widget-loading': !currentStore.hasLoaded }]" @mouseenter="onTitleEnter" @mouseleave="onRootLeave">
    <div data-tauri-drag-region :class="collapsed ? 'relative flex items-center justify-center mb-2' : 'title-bar mb-2'" @mouseenter="onTitleEnter" @mousedown="onTitleDown">
      <ProviderToggle v-if="!collapsed" />
      <span v-if="collapsed" class="inline-flex items-center text-sm font-semibold leading-none">
        <template v-if="currentStore.hasLoaded && currentStore.hasPlatformSession">
          <template v-if="activeProvider === 'mimo' && mimoSettings.paymentMode === 'plan'">
            <span class="text-sm text-gray-600 dark:text-gray-300">套餐</span>
            <span class="text-sm text-orange-500 dark:text-orange-400 ml-1">{{ (mimoStore.planUsagePercent ?? 0).toFixed(1) }}%</span>
          </template>
          <template v-else>
            <span class="text-sm text-gray-600 dark:text-gray-300">今日费用</span>
            <span class="text-sm text-orange-500 dark:text-orange-400 ml-1">¥{{ currentStore.currentDayCost.toFixed(2) }}</span>
          </template>
          <span class="w-px h-[10px] bg-gray-400 dark:bg-gray-400 mx-1.5"></span>
          <span class="text-sm text-gray-600 dark:text-gray-300">{{ collapsedTitleModelLabel }}</span>
          <span class="text-sm text-blue-600 dark:text-blue-400 ml-1">{{ collapsedTitleTokens }}</span>
          <span class="w-px h-[10px] bg-gray-400 dark:bg-gray-400 mx-1.5"></span>
          <span class="text-sm text-gray-600 dark:text-gray-300">命中率</span>
          <span class="text-sm text-purple-400 dark:text-purple-400 ml-1">{{ currentStore.detailTodayCacheHitRate }}</span>
        </template>
        <span v-else-if="currentStore.hasLoaded" class="text-gray-500 dark:text-gray-400">未登录 Platform</span>
      </span>
      <div v-if="!collapsed" class="title-actions">
        <button aria-label="刷新" class="title-btn refresh-btn" @click="currentStore.refresh()"><RefreshIcon /></button>
        <button aria-label="主题" class="title-btn" @click="theme.toggle"><ThemeIcon /></button>
        <button aria-label="设置" class="title-btn settings-btn" @click="openSettings"><SettingsIcon /></button>
        <button aria-label="关闭" class="title-btn close-btn" @click="closeWindow"><CloseIcon /></button>
      </div>
    </div>

    <div ref="contentWrap" v-show="!collapsed" class="space-y-3">
    <div v-if="!currentStore.hasLoaded"></div>

    <!-- Not logged in -->
    <template v-else-if="currentStore.isFirstLaunch || !currentStore.hasPlatformSession">
    <div class="glass-card p-5">
      <div class="text-center mb-3">
        <div class="text-2xl mb-2">{{ currentStore.isFirstLaunch ? '👋' : '⚠️' }}</div>
        <div class="text-sm font-semibold text-gray-700 dark:text-gray-300">{{ currentStore.isFirstLaunch ? '欢迎使用' : '需要登录' }}</div>
        <div class="text-xs text-gray-500 dark:text-gray-400 mt-1">请在主界面完成登录配置</div>
      </div>
      <button @click="openMainWindow" class="w-full px-3 py-2 bg-brand-500 hover:bg-brand-600 text-white text-xs font-semibold rounded-lg transition shadow-sm shadow-brand-500/20">打开主界面</button>
    </div>
    <ErrorBanner v-if="currentStore.errorMessage && !currentStore.isFirstLaunch" class="mt-3" :error="currentStore.errorMessage" :warning="null" />
    </template>

    <!-- DeepSeek Widget -->
    <template v-else-if="activeProvider === 'deepseek'">
    <ErrorBanner :error="dsStore.errorMessage" :warning="dsStore.warningMessage" :showSettings="true" />
    <div class="glass-card p-5">
      <div class="flex items-start justify-between">
        <div class="text-sm font-semibold tracking-wider uppercase text-gray-500 dark:text-gray-400 mb-1">可用余额</div>
        <div class="flex items-center gap-1.5">
          <span class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full font-medium backdrop-blur-sm border border-white/20 dark:border-white/10" :class="dsStore.isAccountAvailable ? 'bg-green-500/10 text-green-700 dark:bg-green-500/15 dark:text-green-400' : 'bg-gray-500/10 text-gray-500 dark:text-gray-400'">
            <span>{{ dsStore.isAccountAvailable ? '✔' : '✖' }}</span><span>{{ dsStore.isAccountAvailable ? '可用' : '不可用' }}</span>
          </span>
          <button @click="invoke('ds_open_top_up_window')" class="inline-flex items-center justify-center gap-1 text-xs px-2 py-1 rounded-full font-medium bg-orange-500/10 text-orange-600 dark:text-orange-400 backdrop-blur-sm border border-white/20 dark:border-white/10 hover:bg-orange-500/20 transition-colors cursor-pointer">
            <svg class="w-3 h-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
            <span class="leading-none">充值</span>
          </button>
        </div>
      </div>
      <div class="text-2xl font-bold text-blue-600 dark:text-blue-400">¥{{ dsStore.totalBalance.toFixed(2) }}</div>
      <div class="grid grid-cols-2 gap-3 mt-3">
        <div class="text-left p-2 rounded-lg border border-white/20 dark:border-white/10 shadow-glass bg-white/10 dark:bg-zinc-900/10 backdrop-blur-sm">
          <div class="text-xs font-semibold tracking-wider uppercase text-gray-500 dark:text-gray-400">今日费用</div>
          <div class="text-xl font-bold text-orange-500 dark:text-orange-400">¥{{ dsStore.currentDayCost.toFixed(2) }}</div>
        </div>
        <div class="text-left p-2 rounded-lg border border-white/20 dark:border-white/10 shadow-glass bg-white/10 dark:bg-zinc-900/10 backdrop-blur-sm">
          <div class="text-xs font-semibold tracking-wider uppercase text-gray-500 dark:text-gray-400">本月费用</div>
          <div class="text-xl font-bold text-orange-500 dark:text-orange-400">¥{{ dsStore.currentMonthCost.toFixed(2) }}</div>
        </div>
      </div>
    </div>
    <TokenDetail />
    </template>

    <!-- MiMo Widget -->
    <template v-else>
    <ErrorBanner :error="mimoStore.errorMessage" :warning="mimoStore.warningMessage" :showSettings="true" />
    <div v-if="mimoSettings.paymentMode === 'plan'" class="glass-card p-5">
      <div class="flex items-start justify-between mb-2">
        <div>
          <div class="text-sm font-semibold tracking-wider uppercase text-gray-500 dark:text-gray-400 mb-1">当前套餐</div>
          <div v-if="mimoStore.planName" class="text-2xl font-bold text-blue-600 dark:text-blue-400">{{ mimoStore.planName }}</div>
          <div v-else class="text-lg font-semibold text-gray-400 dark:text-gray-500">未订阅</div>
        </div>
        <div class="flex items-center gap-1.5">
          <span v-if="mimoStore.planName" class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full font-medium backdrop-blur-sm border border-white/20 dark:border-white/10" :class="mimoStore.planExpired ? 'bg-gray-500/10 text-gray-500 dark:text-gray-400' : 'bg-green-500/10 text-green-700 dark:bg-green-500/15 dark:text-green-400'">
            <span>{{ mimoStore.planExpired ? '✖' : '✔' }}</span><span>{{ mimoStore.planExpired ? '已过期' : '有效' }}</span>
          </span>
          <span v-else class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full font-medium bg-gray-500/10 text-gray-500 dark:text-gray-400 backdrop-blur-sm border border-white/20 dark:border-white/10"><span>—</span><span>未订阅</span></span>
          <button @click="invoke('open_plan_manage')" class="inline-flex items-center justify-center gap-1 text-xs px-2 py-1 rounded-full font-medium bg-orange-500/10 text-orange-600 dark:text-orange-400 backdrop-blur-sm border border-white/20 dark:border-white/10 hover:bg-orange-500/20 transition-colors cursor-pointer">
            <svg class="w-3 h-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
            <span class="leading-none">续费</span>
          </button>
        </div>
      </div>
      <div v-if="mimoStore.planName">
        <div class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400 mb-1.5"><span>套餐用量</span><span class="tabular-nums">{{ mimoStore.planUsed?.toLocaleString() ?? '–' }} / {{ mimoStore.planLimit?.toLocaleString() ?? '–' }} 已使用 {{ (mimoStore.planUsagePercent ?? 0).toFixed(1) }}%</span></div>
        <div class="w-full h-3 rounded-full bg-gray-200 dark:bg-zinc-700 overflow-hidden"><div class="h-full rounded-full transition-all duration-500" :style="{ width: Math.min(mimoStore.planUsagePercent ?? 0, 100) + '%' }" :class="(mimoStore.planUsagePercent ?? 0) > 90 ? 'bg-red-500 dark:bg-red-400' : (mimoStore.planUsagePercent ?? 0) > 70 ? 'bg-orange-400 dark:bg-orange-400' : 'bg-blue-600 dark:bg-blue-400'"></div></div>
        <div v-if="mimoStore.planPeriodEnd" class="text-xs text-gray-400 dark:text-gray-500 mt-2">到期时间：{{ mimoStore.planPeriodEnd }}</div>
      </div>
    </div>
    <BalanceInfoCard v-else />
    <MimoTokenDetail />
    </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch, nextTick } from "vue";
import { getCurrentWindow, monitorFromPoint } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useDashboardStore } from "@/stores/dashboard";
import { useMimoDashboardStore } from "@/stores/mimo-dashboard";
import { useThemeStore } from "@/stores/theme";
import { useSettingsStore } from "@/stores/settings";
import { useSettingsStore as useMimoSettingsStore } from "@/stores/mimo-settings";
import { useProviderStore } from "@/stores/provider";
import TokenDetail from "@/components/TokenDetail.vue";
import MimoTokenDetail from "@/components/MimoTokenDetail.vue";
import ErrorBanner from "@/components/ErrorBanner.vue";
import BalanceInfoCard from "@/components/BalanceInfoCard.vue";
import ProviderToggle from "@/components/ProviderToggle.vue";
import RefreshIcon from "@/components/icons/RefreshIcon.vue";
import ThemeIcon from "@/components/icons/ThemeIcon.vue";
import SettingsIcon from "@/components/icons/SettingsIcon.vue";
import CloseIcon from "@/components/icons/CloseIcon.vue";

const dsStore = useDashboardStore();
const mimoStore = useMimoDashboardStore();
const theme = useThemeStore();
const settings = useSettingsStore();
const mimoSettings = useMimoSettingsStore();
const { activeProvider } = useProviderStore();
const currentLabel = getCurrentWindow().label;
const rootEl = ref<HTMLElement>();
const collapsed = ref(false);
const docked = ref(false);

const currentStore = computed(() => activeProvider.value === "deepseek" ? dsStore : mimoStore);

const collapsedTitleTokens = computed(() => {
  if (activeProvider.value === "deepseek") {
    return (dsStore.selectedDetailModel === "pro" ? dsStore.currentDayProTokens : dsStore.currentDayFlashTokens).toLocaleString();
  } else {
    return (mimoSettings.selectedDetailModel === "v25pro" ? mimoStore.currentDayProTokens : mimoStore.currentDayFlashTokens).toLocaleString();
  }
});
const collapsedTitleModelLabel = computed(() => {
  if (activeProvider.value === "deepseek") return dsStore.selectedDetailModel === "pro" ? "V4 Pro" : "V4.1 Flash";
  return mimoSettings.selectedDetailModel === "v25pro" ? "V2.5 Pro" : "V2.5";
});

watch(() => dsStore.selectedDetailModel, (val) => { settings.setSelectedDetailModel(val); });
watch(() => dsStore.selectedTrendModel, (val) => { settings.setSelectedTrendModel(val); });
watch(() => mimoStore.selectedDetailModel, (val) => { mimoSettings.setSelectedDetailModel(val); });
watch(() => mimoStore.selectedTrendModel, (val) => { mimoSettings.setSelectedTrendModel(val); });

// Collapsed glass effect
watch(collapsed, (val) => {
  if (val) {
    document.documentElement.classList.add("collapsed");
    document.body.classList.add("collapsed");
  } else {
    document.documentElement.classList.remove("collapsed");
    document.body.classList.remove("collapsed");
  }
});

// Reset mouse/drag state when provider switches
watch(activeProvider, () => {
  clearDragGuard();
  mouseInside = false;
});

let mouseInside = false;
let dragGuard = false;
let dragGuardTimer: ReturnType<typeof setTimeout> | null = null;
let resizeObserver: ResizeObserver | null = null;
let unlistenSnap: (() => void) | null = null;
let unlistenRelease: (() => void) | null = null;

function setupResizeObserver() {
  if (!rootEl.value) return;
  if (resizeObserver) resizeObserver.disconnect();
  let pending = false;
  const resize = () => { const h = rootEl.value!.scrollHeight; if (h > 0) getCurrentWindow().setSize(new LogicalSize(400, h)).catch(() => {}); };
  resize();
  resizeObserver = new ResizeObserver(() => { if (pending) return; pending = true; requestAnimationFrame(() => { pending = false; resize(); }); });
  resizeObserver.observe(rootEl.value);
}

function clearDragGuard() { dragGuard = false; if (dragGuardTimer) { clearTimeout(dragGuardTimer); dragGuardTimer = null; } }
function onTitleEnter(e: MouseEvent) { if (dragGuard) return; mouseInside = true; if (e.clientX < 0 || e.clientX >= window.innerWidth) return; if (e.clientY < 0 || e.clientY >= window.innerHeight) return; if (docked.value && collapsed.value && settings.edgeSnapEnabled) collapsed.value = false; }
function onTitleDown() { dragGuard = true; if (dragGuardTimer) clearTimeout(dragGuardTimer); dragGuardTimer = setTimeout(() => { dragGuard = false; }, 2000); }
function onRootLeave() { mouseInside = false; if (dragGuard) return; if (docked.value && !collapsed.value && settings.edgeSnapEnabled) collapsed.value = true; }

onMounted(async () => {
  // Load settings FIRST, then refresh data
  await Promise.all([
    settings.loadEdgeSnapEnabled(),
    settings.loadSelectedDetailModel().then(val => { dsStore.selectedDetailModel = val; }),
    settings.loadSelectedTrendModel().then(val => { dsStore.selectedTrendModel = val; }),
    mimoSettings.loadPaymentMode(),
    mimoSettings.loadEdgeSnapEnabled(),
    mimoSettings.loadSelectedDetailModel().then(val => { mimoStore.selectedDetailModel = val; }),
    mimoSettings.loadSelectedTrendModel().then(val => { mimoStore.selectedTrendModel = val; }),
  ]);
  const [dsSnapshot, mimoSnapshot] = await Promise.all([
    invoke<any | null>("ds_get_cached_data"),
    invoke<any | null>("mimo_get_cached_data"),
  ]).catch(() => [null, null]);
  if (dsSnapshot) dsStore.applyData(dsSnapshot);
  else dsStore.refresh();
  if (mimoSnapshot) mimoStore.applyData(mimoSnapshot);
  else mimoStore.refresh();

  // DS events (RAW used "login-complete" and "trigger-refresh")
  listen("detail-model-changed", (event: { payload: any }) => {
    if (event.payload === "pro" || event.payload === "flash") dsStore.selectedDetailModel = event.payload;
    else if (event.payload === "v25pro" || event.payload === "v25") mimoStore.selectedDetailModel = event.payload;
  });
  listen("ds-login-complete", () => { invoke("ds_broadcast_refresh"); });
  listen("ds-trigger-refresh", (event: { payload: any }) => { if (event.payload && typeof event.payload === 'object' && 'has_platform_session' in event.payload) dsStore.applyData(event.payload); else dsStore.refresh(); });

  // MiMo events
  listen("mimo-login-complete", () => { invoke("mimo_broadcast_refresh"); });
  listen("mimo-trigger-refresh", (event: { payload: any }) => { if (event.payload && typeof event.payload === 'object' && 'has_platform_session' in event.payload) mimoStore.applyData(event.payload); else mimoStore.refresh(); });
  listen("payment-mode-changed", (event: { payload: "plan" | "token" }) => { mimoSettings.paymentMode = event.payload; });

  // Shared
  listen("edge-snap-setting-changed", (event: { payload: boolean }) => {
    settings.edgeSnapEnabled = event.payload;
    if (event.payload) { getCurrentWindow().outerPosition().then(async pos => { const mt = (await monitorFromPoint(pos.x, pos.y))?.position.y ?? 0; if (pos.y - mt <= 10 && !collapsed.value) { docked.value = true; collapsed.value = true; } }).catch(() => {}); }
    else if (docked.value) { docked.value = false; if (collapsed.value) collapsed.value = false; }
  });

  const [_snap, u1, u2] = await Promise.all([
    settings.loadEdgeSnapEnabled(),
    getCurrentWindow().listen<{ label: string }>("edge-snap-triggered", (event) => { if (event.payload.label !== currentLabel) return; clearDragGuard(); if (settings.edgeSnapEnabled) { docked.value = true; collapsed.value = true; } }),
    getCurrentWindow().listen<{ label: string }>("edge-snap-released", (event) => { if (event.payload.label !== currentLabel) return; clearDragGuard(); docked.value = false; if (collapsed.value) collapsed.value = false; }),
  ]);
  unlistenSnap = u1; unlistenRelease = u2;

  if (settings.edgeSnapEnabled) {
    try { const pos = await getCurrentWindow().outerPosition(); const mt = (await monitorFromPoint(pos.x, pos.y))?.position.y ?? 0; if (pos.y - mt <= 10) { docked.value = true; collapsed.value = true; } } catch (_) {}
  }

  await nextTick(); setupResizeObserver();
});

onUnmounted(() => { if (resizeObserver) resizeObserver.disconnect(); if (dragGuardTimer) clearTimeout(dragGuardTimer); if (unlistenSnap) unlistenSnap(); if (unlistenRelease) unlistenRelease(); });

function closeWindow() { invoke("hide_widget_window"); }
function openSettings() { invoke("show_settings_window"); }
function openMainWindow() { invoke("show_main_window"); }
</script>
