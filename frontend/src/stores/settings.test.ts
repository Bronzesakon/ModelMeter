import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { invokeMock, emitMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  emitMock: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ emit: emitMock }));

import { useSettingsStore } from "./settings";

describe("settings store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    emitMock.mockReset();
    invokeMock.mockResolvedValue(null);
  });

  it("setRefreshInterval updates state and persists it", () => {
    const store = useSettingsStore();
    store.setRefreshInterval(120);
    expect(store.refreshInterval).toBe(120);
    expect(invokeMock).toHaveBeenCalledWith("save_setting", {
      key: "refresh_interval",
      value: "120",
    });
  });

  it("loadRefreshInterval parses a persisted value", async () => {
    invokeMock.mockResolvedValue("90");
    const store = useSettingsStore();
    await store.loadRefreshInterval();
    expect(store.refreshInterval).toBe(90);
  });

  it("loadRefreshInterval falls back to 60 when unset", async () => {
    const store = useSettingsStore();
    await store.loadRefreshInterval();
    expect(store.refreshInterval).toBe(60);
  });

  it("loadRefreshInterval ignores invalid and non-positive values", async () => {
    const store = useSettingsStore();
    invokeMock.mockResolvedValue("abc");
    await store.loadRefreshInterval();
    expect(store.refreshInterval).toBe(60);

    invokeMock.mockResolvedValue("0");
    await store.loadRefreshInterval();
    expect(store.refreshInterval).toBe(60);
  });

  it("loadEdgeSnapEnabled only accepts the string true", async () => {
    const store = useSettingsStore();
    invokeMock.mockResolvedValue("true");
    await store.loadEdgeSnapEnabled();
    expect(store.edgeSnapEnabled).toBe(true);

    invokeMock.mockResolvedValue("false");
    await store.loadEdgeSnapEnabled();
    expect(store.edgeSnapEnabled).toBe(false);
  });

  it("selected detail model defaults to flash for unknown values", async () => {
    const store = useSettingsStore();
    invokeMock.mockResolvedValue("weird");
    await expect(store.loadSelectedDetailModel()).resolves.toBe("flash");

    invokeMock.mockResolvedValue("flash");
    await expect(store.loadSelectedDetailModel()).resolves.toBe("flash");

    invokeMock.mockResolvedValue("v4flash");
    await expect(store.loadSelectedDetailModel()).resolves.toBe("v4flash");
  });

  it("legacy persisted pro selection migrates to v4flash", async () => {
    const store = useSettingsStore();
    invokeMock.mockResolvedValue("pro");
    await expect(store.loadSelectedDetailModel()).resolves.toBe("v4flash");
    await expect(store.loadSelectedTrendModel()).resolves.toBe("v4flash");
  });

  it("setSelectedDetailModel emits only when the value changes", async () => {
    const store = useSettingsStore();
    await store.setSelectedDetailModel("v4flash");
    expect(store.selectedDetailModel).toBe("v4flash");
    expect(emitMock).toHaveBeenCalledWith("detail-model-changed", "v4flash");

    emitMock.mockClear();
    await store.setSelectedDetailModel("v4flash");
    expect(emitMock).not.toHaveBeenCalled();
  });

  it("setAutoStart applies optimistically on success", async () => {
    const store = useSettingsStore();
    invokeMock.mockResolvedValue(undefined);
    await store.setAutoStart(true);
    expect(store.autoStart).toBe(true);
    expect(store.autoStartPending).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("set_auto_start", { enabled: true });
  });

  it("setAutoStart rolls back when the backend rejects", async () => {
    const store = useSettingsStore();
    invokeMock.mockRejectedValue(new Error("denied"));
    await store.setAutoStart(true);
    expect(store.autoStart).toBe(false);
    expect(store.autoStartPending).toBe(false);
  });
});
