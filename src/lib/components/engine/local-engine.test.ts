import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";

const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke
}));

import LocalEngine from "./local-engine.svelte";

function mockConfig() {
  return {
    tts: {
      active_backend: "openai",
      preset: "piper",
      command: "python3",
      args_template: ["-m", "piper", "{input}", "{output}"],
      voice: "en_US-joe-medium",
      openai: { api_key: "", model: "tts-1", voice: "alloy" },
      elevenlabs: {
        api_key: "",
        voice_id: "21m00Tcm4TlvDq8ikWAM",
        model_id: "eleven_turbo_v2_5",
        output_format: "mp3_44100_128",
        voice_stability: 0.5,
        voice_similarity_boost: 0.75
      },
      cartesia: {
        api_key: "",
        model_id: "sonic-3.5",
        voice_id: "f786b574-daa5-4673-aa0c-cbe3e8534c02",
        voice_name: "Katie",
        output_format: "wav",
        use_manual_voice_id: false
      }
    }
  } as any;
}

describe("LocalEngine", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((command) => {
      if (command === "get_data_dir") return Promise.resolve("%APPDATA%\\CopySpeak");
      if (command === "get_home_dir") return Promise.resolve("%USERPROFILE%");
      return Promise.resolve({ success: true, message: "Engine is working" });
    });
  });

  it("does not edit local voice on the engine page", () => {
    render(LocalEngine, { localConfig: mockConfig() });

    expect(document.getElementById("tts-voice")).toBeNull();
  });

  it("tests the local engine config directly", async () => {
    render(LocalEngine, { localConfig: mockConfig() });

    await fireEvent.click(screen.getByText("Test Engine"));

    expect(mockInvoke).toHaveBeenCalledWith("test_tts_engine_config", {
      engine: "local",
      preset: "piper"
    });
  });
});
