import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import AppHeader from "./app-header.svelte";

const { listeners, unlistened, ipc, mockInvoke } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => Promise<void> | void>(),
  unlistened: new Set<() => void>(),
  ipc: {
    listenGate: Promise.resolve(),
    configResolvers: [] as Array<(config: unknown) => void>
  },
  mockInvoke: vi.fn()
}));

vi.mock("#lib/services/tauri.js", () => ({ isTauri: true }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (name: string, callback: (event: { payload: unknown }) => Promise<void> | void) => {
      await ipc.listenGate;
      listeners.set(name, callback);
      const unlistenFn = () => {
        listeners.delete(name);
        unlistened.add(unlistenFn);
      };
      return unlistenFn;
    }
  )
}));

function makeConfig(experimental: boolean, history: boolean) {
  return {
    general: { experimental_enabled: experimental },
    history: { enabled: history },
    playback: { duck: { enabled: false, level_percent: 20 }, streaming_enabled: true }
  };
}

describe("app-header history gating", () => {
  beforeEach(async () => {
    await waitForI18nReady();
    listeners.clear();
    unlistened.clear();
    ipc.listenGate = Promise.resolve();
    ipc.configResolvers = [];
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders history nav item only when both experimental and history are enabled", async () => {
    mockInvoke.mockResolvedValueOnce(makeConfig(true, true));
    const view = render(AppHeader);

    await waitFor(() => {
      expect(view.queryByTestId("nav-history")).not.toBeNull();
    });
    view.unmount();
  });

  it("omits history nav item when experimental is off even if history is enabled", async () => {
    mockInvoke.mockResolvedValueOnce(makeConfig(false, true));
    const view = render(AppHeader);

    await waitFor(() => {
      expect(view.queryByTestId("nav-generate")).not.toBeNull();
    });
    expect(view.queryByTestId("nav-history")).toBeNull();
    view.unmount();
  });

  it("omits history nav item when history is disabled even if experimental is on", async () => {
    mockInvoke.mockResolvedValueOnce(makeConfig(true, false));
    const view = render(AppHeader);

    await waitFor(() => {
      expect(view.queryByTestId("nav-generate")).not.toBeNull();
    });
    expect(view.queryByTestId("nav-history")).toBeNull();
    view.unmount();
  });

  it("updates history link visibility when config-changed event fires", async () => {
    mockInvoke.mockResolvedValueOnce(makeConfig(false, true));
    const view = render(AppHeader);

    await waitFor(() => {
      expect(view.queryByTestId("nav-history")).toBeNull();
    });

    // Now simulate enabling experimental in config and config-changed firing
    mockInvoke.mockResolvedValueOnce(makeConfig(true, true));
    const configChangedListener = listeners.get("config-changed");
    expect(configChangedListener).toBeDefined();

    await configChangedListener!({ payload: null });

    await waitFor(() => {
      expect(view.queryByTestId("nav-history")).not.toBeNull();
    });
    view.unmount();
  });

  it("discards stale config fetch responses when a newer fetch resolves first", async () => {
    let resolveFirst!: (val: unknown) => void;
    const firstFetchPromise = new Promise((resolve) => {
      resolveFirst = resolve;
    });

    // First fetch hangs
    mockInvoke.mockImplementationOnce(() => firstFetchPromise);
    const view = render(AppHeader);

    // Ensure listen is registered
    await waitFor(() => {
      expect(listeners.get("config-changed")).toBeDefined();
    });

    // Second fetch (from config-changed) resolves quickly with history effective = true
    mockInvoke.mockResolvedValueOnce(makeConfig(true, true));
    const onConfigChanged = listeners.get("config-changed");
    await onConfigChanged!({ payload: null });

    await waitFor(() => {
      expect(view.queryByTestId("nav-history")).not.toBeNull();
    });

    // Now the stale first fetch resolves with history effective = false
    resolveFirst(makeConfig(false, false));
    await Promise.resolve();

    // The newer result (history enabled) should NOT have been overwritten by stale fetch
    expect(view.queryByTestId("nav-history")).not.toBeNull();
    view.unmount();
  });

  it("cleans up late-resolving config-changed listener if component is destroyed mid-onMount", async () => {
    let resolveListenGate!: () => void;
    ipc.listenGate = new Promise((resolve) => {
      resolveListenGate = resolve;
    });
    mockInvoke.mockResolvedValue(makeConfig(true, true));

    const view = render(AppHeader);
    // Unmount while listenGate is still pending
    view.unmount();

    // Now the listen resolves after unmount
    resolveListenGate();
    await waitFor(() => {
      expect(unlistened.size).toBe(1);
    });
  });
});
