import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { invokeMock, emitMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  emitMock: vi.fn(),
  listenMock: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ emit: emitMock, listen: listenMock }));

import { useThemeStore } from "./theme";

function mockPrefersDark(dark: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: dark,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })) as unknown as typeof window.matchMedia;
}

describe("theme store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    emitMock.mockReset();
    listenMock.mockReset();
    invokeMock.mockResolvedValue(null);
    listenMock.mockResolvedValue(() => {});
    document.documentElement.classList.remove("dark");
    mockPrefersDark(false);
  });

  it("toggle goes light -> dark, saves and broadcasts", async () => {
    const store = useThemeStore();
    store.toggle();
    expect(store.mode).toBe("dark");
    expect(store.isDark).toBe(true);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("save_setting", {
      key: "app_theme",
      value: "dark",
    });
    expect(emitMock).toHaveBeenCalledWith("theme-mode-changed", "dark");
  });

  it("toggle goes dark -> light and removes the class", async () => {
    const store = useThemeStore();
    store.setMode("dark");
    store.toggle();
    expect(store.mode).toBe("light");
    expect(store.isDark).toBe(false);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("setMode(system) follows the system preference", async () => {
    mockPrefersDark(true);
    const store = useThemeStore();
    store.setMode("system");
    expect(store.mode).toBe("system");
    expect(store.isDark).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("save_setting", {
      key: "app_theme",
      value: "system",
    });
  });

  it("init applies a persisted dark theme", async () => {
    invokeMock.mockResolvedValue("dark");
    const store = useThemeStore();
    store.init();
    await vi.waitFor(() => expect(store.mode).toBe("dark"));
    expect(store.isDark).toBe(true);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("init defaults to light when nothing is persisted", async () => {
    const store = useThemeStore();
    store.init();
    await vi.waitFor(() => expect(listenMock).toHaveBeenCalled());
    expect(store.mode).toBe("light");
    expect(store.isDark).toBe(false);
  });
});
