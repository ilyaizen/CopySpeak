import { describe, expect, it, vi, beforeEach, afterEach } from "vite-plus/test";
import { render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import HistoryRoute from "./+page.svelte";
import { goto } from "$app/navigation";
import { invoke } from "#lib/services/tauri";

vi.mock("#lib/services/tauri", () => ({
  isTauri: true,
  invoke: vi.fn()
}));

vi.mock("#lib/stores/history-store.svelte.js", () => ({
  historyStore: {
    items: [
      {
        id: "secret-1",
        timestamp: Date.now(),
        text: "Confidential User Reading",
        tts_engine: "cartesia",
        voice: "saved",
        speed: 1,
        success: true,
        output_path: "/audio/1.wav"
      }
    ],
    loadHistory: vi.fn()
  }
}));

function makeConfig(experimental: boolean, history: boolean) {
  return {
    general: { experimental_enabled: experimental },
    history: { enabled: history }
  };
}

describe("history route experimental gating", () => {
  beforeEach(async () => {
    await waitForI18nReady();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders history items when both experimental and history are enabled", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeConfig(true, true));
    const view = render(HistoryRoute);

    await waitFor(() => {
      expect(view.queryByText(/Confidential User Reading/)).not.toBeNull();
    });
    expect(goto).not.toHaveBeenCalled();
    expect(view.queryByTestId("history-inactive")).toBeNull();
    view.unmount();
  });

  it("redirects home and shows no private saved data when experimental is off", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeConfig(false, true));
    const view = render(HistoryRoute);

    await waitFor(() => {
      expect(goto).toHaveBeenCalledWith("/", { replaceState: true });
    });
    expect(view.queryByText(/Confidential User Reading/)).toBeNull();
    expect(view.queryByTestId("history-inactive")).not.toBeNull();
    view.unmount();
  });

  it("redirects home and shows no private saved data when history is off", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeConfig(true, false));
    const view = render(HistoryRoute);

    await waitFor(() => {
      expect(goto).toHaveBeenCalledWith("/", { replaceState: true });
    });
    expect(view.queryByText(/Confidential User Reading/)).toBeNull();
    expect(view.queryByTestId("history-inactive")).not.toBeNull();
    view.unmount();
  });
});
