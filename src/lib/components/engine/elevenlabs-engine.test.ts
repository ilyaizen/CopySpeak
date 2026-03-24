import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";

// Mock Tauri IPC before importing components
const mockInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke
}));

import ElevenLabsEngine from "./elevenlabs-engine.svelte";

describe("ElevenLabsEngine", () => {
  const mockConfig = {
    trigger: { listen_enabled: true, double_copy_window_ms: 1000, max_text_length: 1000 },
    tts: {
      active_backend: "elevenlabs",
      preset: "piper",
      command: "python3",
      args_template: ["-m", "piper"],
      voice: "alloy",
      openai: { api_key: "", model: "tts-1", voice: "alloy" },
      elevenlabs: {
        api_key: "xi-test",
        voice_id: "21m00Tcm4TlvDq8ikWAM",
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
      show_notifications: true,
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
  };

  let bindableConfig: any;

  beforeEach(() => {
    vi.clearAllMocks();
    // Create a bindable config object
    bindableConfig = {
      get: () => mockConfig,
      set: (val: any) => Object.assign(bindableConfig, val),
      ...mockConfig
    };
  });

  it("ENG-03: test button appears when elevenlabs backend is active", () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.queryByText("Test Engine");
    expect(testButton).toBeTruthy();
  });

  it("ENG-03: test button does not appear when elevenlabs backend is not active", () => {
    bindableConfig.tts.active_backend = "openai";
    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.queryByText("Test Engine");
    expect(testButton).toBeNull();
  });

  it("ENG-03: test button calls test_tts_engine IPC", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({ success: true, message: "Engine is working" });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);

    expect(mockInvoke).toHaveBeenCalledWith("test_tts_engine");
  });

  it('ENG-03: shows "Testing..." while loading', async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    let resolveTest: any;
    mockInvoke.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTest = resolve;
        })
    );

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);

    // Button should show "Testing..." while loading
    expect(screen.queryByText("Test Engine")).toBeNull();
    expect(screen.queryByText("Testing...")).toBeTruthy();

    // Resolve the promise
    resolveTest({ success: true, message: "Engine is working" });
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

  it('ENG-03: success shows green alert with "Engine is working"', async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({ success: true, message: "Engine is working" });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Check for success alert title (may appear in title and description)
    const matches = screen.getAllByText("Engine is working");
    expect(matches.length).toBeGreaterThan(0);
  });

  it("ENG-03: failure shows red alert with specific error message", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "API key is missing or invalid",
      error_type: "api_key_missing"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("API key is missing or invalid")).toBeTruthy();
  });

  it("ENG-03: displays correct error for auth_failed", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Authentication failed. Check your API key.",
      error_type: "auth_failed"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Authentication failed. Check your API key.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for rate_limit", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Rate limit exceeded. Please try again later.",
      error_type: "rate_limit"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Rate limit exceeded. Please try again later.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for http_error", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Network error. Check your internet connection.",
      error_type: "http_error"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Network error. Check your internet connection.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for not_found", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Command not found. Install the TTS engine.",
      error_type: "not_found"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Command not found. Install the TTS engine.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for permission_denied", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Permission denied. Check file permissions.",
      error_type: "permission_denied"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Permission denied. Check file permissions.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for unavailable", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Engine unavailable. Check configuration.",
      error_type: "unavailable"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Engine unavailable. Check configuration.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for io_error", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "I/O error. Check file paths and permissions.",
      error_type: "io_error"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("I/O error. Check file paths and permissions.")).toBeTruthy();
  });

  it("ENG-03: displays correct error for unknown", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Unknown error. Check logs for details.",
      error_type: "unknown"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(screen.getByText("Unknown error. Check logs for details.")).toBeTruthy();
  });

  it("ENG-03: install card does NOT appear for elevenlabs backend", async () => {
    bindableConfig.tts.active_backend = "elevenlabs";
    mockInvoke.mockResolvedValue({
      success: false,
      message: "Command not found. Install the TTS engine.",
      error_type: "not_found"
    });

    render(ElevenLabsEngine, { localConfig: bindableConfig });

    const testButton = screen.getByText("Test Engine");
    await fireEvent.click(testButton);
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Install card should NOT appear for ElevenLabs backend
    expect(screen.queryByText("Install TTS Engine")).toBeNull();
  });
});
