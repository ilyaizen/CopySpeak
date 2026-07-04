import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));

import EnginePage from "./engine-page.svelte";

// Minimal config the unified engine page reads. The page only touches
// credentials now; voice/model fields are intentionally absent to prove the
// engine page no longer depends on them.
function mockConfig() {
  return {
    tts: {
      active_backend: "edge",
      active_profile_id: "default",
      profiles: [{ id: "default", name: "Edge - Ava", engine: "edge", voice: "en-US-AvaNeural", speed: 1, pitch: 1, effects: { enabled: false, active_effect: "none" } }],
      openai: { api_key: "" },
      elevenlabs: { api_key: "" },
      cartesia: { api_key: "" },
      google: { api_key: "" },
      microsoft: { api_key: "", endpoint: "" }
    }
  } as any;
}

describe("EnginePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((command: string) => {
      if (command === "get_config") return Promise.resolve(mockConfig());
      if (command === "check_command_exists") return Promise.resolve({ available: true });
      return Promise.resolve({});
    });
  });

  it("renders one nav tab per registered engine", async () => {
    const { container } = render(EnginePage);
    await waitFor(() => {
      const tabs = container.querySelectorAll("aside nav button");
      // edge, cartesia, elevenlabs, openai, google, microsoft,
      // kitten, piper, kokoro, pocket, chatterbox, http
      expect(tabs).toHaveLength(12);
    });
  });

  it("loads config and checks uv availability on mount", async () => {
    render(EnginePage);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_config");
      expect(mockInvoke).toHaveBeenCalledWith("check_command_exists", { command: "uv" });
    });
  });

  it("does not render voice or model controls on the engine page", async () => {
    const { container } = render(EnginePage);
    await waitFor(() => expect(container.querySelectorAll("aside nav button").length).toBe(12));
    // No selects/sliders on this page — those belong to profiles.
    expect(container.querySelector("select")).toBeNull();
    expect(container.querySelector('input[type="range"]')).toBeNull();
  });
});
