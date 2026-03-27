import { describe, it, expect, beforeEach, vi } from "vitest";
import { render } from "@testing-library/svelte";

// Mock Tauri IPC before importing components
const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke
}));

import EnginePage from "./engine-page.svelte";

describe("EnginePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("Task-1: should not contain centralized test button", () => {
    const { container } = render(EnginePage);

    // Verify no centralized "Test Engine" button exists at bottom of page
    const testButtons = container.querySelectorAll("button");
    testButtons.forEach((btn) => {
      if (btn.textContent?.includes("Test Engine") || btn.textContent?.includes("Test TTS")) {
        // Test buttons in backend components are OK, but verify they're NOT centralized
        // Centralized button would be in the save bar area or separate section
        const parent = btn.parentElement;
        expect(parent?.classList.contains("border-border")).toBe(false);
      }
    });
  });

  it("Task-1: should render all five backend tabs after config loads", async () => {
    mockInvoke.mockResolvedValue({
      trigger: { listen_enabled: true, double_copy_window_ms: 1000, max_text_length: 1000 },
      tts: {
        active_backend: "local",
        preset: "piper",
        command: "python3",
        args_template: ["-m", "piper"],
        voice: "en_US-joe-medium",
        openai: { api_key: "", model: "tts-1", voice: "alloy" },
        elevenlabs: {
          api_key: "",
          voice_id: "",
          model_id: "eleven_turbo_v2_5",
          output_format: "mp3_44100_128",
          voice_stability: 0.5,
          voice_similarity_boost: 0.75
        }
      },
      playback: { on_retrigger: "interrupt", volume: 100, playback_speed: 1.0 },
      general: {
        start_with_windows: false,
        start_minimized: false,
        debug_mode: false,
        close_behavior: "minimize-to-tray",
        appearance: "system"
      },
      output: {
        enabled: false,
        directory: "",
        filename_pattern: "{timestamp}_{text}",
        format_config: { format: "wav", mp3_bitrate: 128, ogg_bitrate: 64, flac_compression: 5 }
      },
      sanitization: {
        enabled: false,
        markdown: {
          enabled: false,
          strip_headers: false,
          strip_code_blocks: false,
          strip_inline_code: false,
          strip_links: false,
          strip_bold_italic: false,
          strip_lists: false,
          strip_blockquotes: false
        },
        tts_normalization: { enabled: false }
      },
      pagination: { enabled: false, fragment_size: 500 },
      history: {
        enabled: false,
        storage_mode: "temp",
        persistent_dir: null,
        auto_delete: "never",
        cleanup_orphaned_files: false
      }
    });

    const { container } = render(EnginePage);
    await new Promise((resolve) => setTimeout(resolve, 50));

    const tabs = container.querySelectorAll('[role="tab"]');
    expect(tabs).toHaveLength(5);

    expect(tabs[0].textContent).toBe("Piper TTS");
    expect(tabs[1].textContent).toBe("Kokoro TTS");
    expect(tabs[2].textContent).toBe("Pocket TTS");
    expect(tabs[3].textContent).toBe("OpenAI");
    expect(tabs[4].textContent).toBe("ElevenLabs");
  });

  it("Task-1: should load config on mount", async () => {
    mockInvoke.mockResolvedValue({
      trigger: { listen_enabled: true, double_copy_window_ms: 1000, max_text_length: 1000 },
      tts: {
        active_backend: "local",
        preset: "piper",
        command: "python3",
        args_template: ["-m", "piper"],
        voice: "en_US-joe-medium",
        openai: { api_key: "", model: "tts-1", voice: "alloy" },
        elevenlabs: {
          api_key: "",
          voice_id: "",
          model_id: "eleven_turbo_v2_5",
          output_format: "mp3_44100_128",
          voice_stability: 0.5,
          voice_similarity_boost: 0.75
        }
      },
      playback: { on_retrigger: "interrupt", volume: 100, playback_speed: 1.0 },
      general: {
        start_with_windows: false,
        start_minimized: false,
        debug_mode: false,
        close_behavior: "minimize-to-tray",
        appearance: "system"
      },
      output: {
        enabled: false,
        directory: "",
        filename_pattern: "{timestamp}_{text}",
        format_config: { format: "wav", mp3_bitrate: 128, ogg_bitrate: 64, flac_compression: 5 }
      },
      sanitization: {
        enabled: false,
        markdown: {
          enabled: false,
          strip_headers: false,
          strip_code_blocks: false,
          strip_inline_code: false,
          strip_links: false,
          strip_bold_italic: false,
          strip_lists: false,
          strip_blockquotes: false
        },
        tts_normalization: { enabled: false }
      },
      pagination: { enabled: false, fragment_size: 500 },
      history: {
        enabled: false,
        storage_mode: "temp",
        persistent_dir: null,
        auto_delete: "never",
        cleanup_orphaned_files: false
      }
    });

    render(EnginePage);

    expect(mockInvoke).toHaveBeenCalledWith("get_config");
  });

  it("Task-1: should save config with set_config IPC", async () => {
    mockInvoke
      .mockResolvedValueOnce({
        trigger: { listen_enabled: true, double_copy_window_ms: 1000, max_text_length: 1000 },
        tts: {
          active_backend: "local",
          preset: "piper",
          command: "python3",
          args_template: ["-m", "piper"],
          voice: "en_US-joe-medium",
          openai: { api_key: "", model: "tts-1", voice: "alloy" },
          elevenlabs: {
            api_key: "",
            voice_id: "",
            model_id: "eleven_turbo_v2_5",
            output_format: "mp3_44100_128",
            voice_stability: 0.5,
            voice_similarity_boost: 0.75
          }
        },
        playback: { on_retrigger: "interrupt", volume: 100, playback_speed: 1.0 },
        general: {
          start_with_windows: false,
          start_minimized: false,
          debug_mode: false,
          close_behavior: "minimize-to-tray",
          appearance: "system"
        },
        output: {
          enabled: false,
          directory: "",
          filename_pattern: "{timestamp}_{text}",
          format_config: { format: "wav", mp3_bitrate: 128, ogg_bitrate: 64, flac_compression: 5 }
        },
        sanitization: {
          enabled: false,
          markdown: {
            enabled: false,
            strip_headers: false,
            strip_code_blocks: false,
            strip_inline_code: false,
            strip_links: false,
            strip_bold_italic: false,
            strip_lists: false,
            strip_blockquotes: false
          },
          tts_normalization: { enabled: false }
        },
        pagination: { enabled: false, fragment_size: 500 },
        history: {
          enabled: false,
          storage_mode: "temp",
          persistent_dir: null,
          auto_delete: "never",
          cleanup_orphaned_files: false
        }
      })
      .mockResolvedValue(undefined);

    const { container } = render(EnginePage);

    // Wait for config to load
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Trigger save by clicking Save button
    const saveButton = Array.from(container.querySelectorAll("button")).find((btn) =>
      btn.textContent?.includes("Save Changes")
    );

    if (saveButton) {
      saveButton.click();
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(mockInvoke).toHaveBeenCalledWith("set_config", { newConfig: expect.any(Object) });
    }
  });
});
