import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";

export const useSettingsStore = defineStore("settings", () => {
  const hasPlatformSession = ref(false);
  const refreshInterval = ref(60);

  const edgeSnapEnabled = ref(true); // default true, 避免首次渲染时展开/收起闪烁
  const autoStart = ref(false);
  const autoStartPending = ref(false); // 正在操作中，禁止重复点击

  // 模型单选按钮持久化（v4flash = V4 Flash 旧名称桶，flash = V4.1 Flash 新模型）
  const selectedDetailModel = ref<"v4flash" | "flash">("flash");
  const selectedTrendModel = ref<"v4flash" | "flash">("flash");

  async function openPlatformLogin() {
    await invoke("ds_open_login_window");
  }

  async function clearPlatformSession() {
    await invoke("ds_clear_platform_session");
    try {
      await invoke("plugin:core|clear_all_browsing_data");
    } catch (_) {}
    hasPlatformSession.value = false;
    invoke("ds_broadcast_refresh");
  }

  function setRefreshInterval(interval: number) {
    console.log("[settings] 刷新间隔变更为 " + interval + " 秒", new Date().toLocaleTimeString());
    refreshInterval.value = interval;
    invoke("save_setting", { key: "refresh_interval", value: String(interval) });
  }

  async function loadRefreshInterval() {
    const val = await invoke<string | null>("load_setting", { key: "refresh_interval" });
    const parsed = val !== null ? parseInt(val, 10) : 60;
    if (!isNaN(parsed) && parsed > 0) {
      refreshInterval.value = parsed;
    }
  }

  async function loadEdgeSnapEnabled() {
    const val = await invoke<string | null>("load_setting", { key: "edge_snap_enabled" });
    edgeSnapEnabled.value = val === "true";
  }

  async function setEdgeSnapEnabled(enabled: boolean) {
    edgeSnapEnabled.value = enabled;
    await invoke("save_setting", { key: "edge_snap_enabled", value: String(enabled) });
    await invoke("broadcast_edge_snap_setting", { enabled });
  }

  function normalizeModelSelection(val: string | null): "v4flash" | "flash" {
    if (val === "flash" || val === "v4flash") return val;
    // 旧版持久化的 "pro"（原 V4 Pro 桶）迁移到 v4flash（旧名称桶），其余按默认
    if (val === "pro") return "v4flash";
    return "flash";
  }

  async function loadSelectedDetailModel(): Promise<"v4flash" | "flash"> {
    const val = await invoke<string | null>("load_setting", { key: "selected_detail_model" });
    selectedDetailModel.value = normalizeModelSelection(val);
    return selectedDetailModel.value;
  }

  async function setSelectedDetailModel(value: "v4flash" | "flash") {
    if (selectedDetailModel.value === value) return;
    selectedDetailModel.value = value;
    await invoke("save_setting", { key: "selected_detail_model", value });
    await emit("detail-model-changed", value);
  }

  async function loadSelectedTrendModel(): Promise<"v4flash" | "flash"> {
    const val = await invoke<string | null>("load_setting", { key: "selected_trend_model" });
    selectedTrendModel.value = normalizeModelSelection(val);
    return selectedTrendModel.value;
  }

  async function setSelectedTrendModel(value: "v4flash" | "flash") {
    if (selectedTrendModel.value === value) return;
    selectedTrendModel.value = value;
    await invoke("save_setting", { key: "selected_trend_model", value });
  }

  async function loadAutoStart() {
    try {
      autoStart.value = await invoke<boolean>("get_auto_start");
    } catch (_) {
      autoStart.value = false;
    }
  }

  async function setAutoStart(enabled: boolean) {
    if (autoStartPending.value) return;
    autoStartPending.value = true;
    // 乐观更新：立即切换按钮，不等待后端
    const prev = autoStart.value;
    autoStart.value = enabled;
    try {
      await invoke("set_auto_start", { enabled });
    } catch (_) {
      // 后端验证失败，回退到之前的状态
      autoStart.value = prev;
    } finally {
      autoStartPending.value = false;
    }
  }

  return {
    hasPlatformSession, refreshInterval, edgeSnapEnabled,
    selectedDetailModel, selectedTrendModel,
    autoStart, autoStartPending,
    openPlatformLogin, clearPlatformSession,
    setRefreshInterval, loadRefreshInterval,
    loadEdgeSnapEnabled, setEdgeSnapEnabled,
    loadSelectedDetailModel, setSelectedDetailModel,
    loadSelectedTrendModel, setSelectedTrendModel,
    loadAutoStart, setAutoStart,
  };
});