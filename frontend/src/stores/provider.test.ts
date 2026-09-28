import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

// useProviderStore 返回的是模块级 ref（并非 defineStore），断言需取 .value；
// 重置模块以获得干净的初始状态。
async function freshStore() {
  vi.resetModules();
  const mod = await import("./provider");
  setActivePinia(createPinia());
  return mod.useProviderStore();
}

describe("provider store", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(null);
  });

  it("defaults to deepseek", async () => {
    const store = await freshStore();
    expect(store.activeProvider.value).toBe("deepseek");
  });

  it("init switches to mimo when the persisted value is mimo", async () => {
    invokeMock.mockResolvedValue("mimo");
    const store = await freshStore();
    await store.init();
    expect(store.activeProvider.value).toBe("mimo");
    expect(invokeMock).toHaveBeenCalledWith("load_setting", { key: "active_provider" });
  });

  it("init keeps deepseek for any other persisted value", async () => {
    invokeMock.mockResolvedValue("weird");
    const store = await freshStore();
    await store.init();
    expect(store.activeProvider.value).toBe("deepseek");
  });

  it("setProvider persists the choice", async () => {
    const store = await freshStore();
    store.setProvider("mimo");
    expect(store.activeProvider.value).toBe("mimo");
    expect(invokeMock).toHaveBeenCalledWith("save_setting", {
      key: "active_provider",
      value: "mimo",
    });
  });

  it("toggle flips between the two providers", async () => {
    const store = await freshStore();
    store.toggle();
    expect(store.activeProvider.value).toBe("mimo");
    store.toggle();
    expect(store.activeProvider.value).toBe("deepseek");
  });

  it("init tolerates a failing backend", async () => {
    invokeMock.mockRejectedValue(new Error("no tauri"));
    const store = await freshStore();
    await expect(store.init()).resolves.toBeUndefined();
    expect(store.activeProvider.value).toBe("deepseek");
  });
});
