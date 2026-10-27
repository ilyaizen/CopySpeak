import type { AppConfig } from "#lib/types";

/** Isolated config for rendered tests; never reads or writes the user's config. */
export function testConfig(): AppConfig {
  return {
    trigger: { listen_enabled: true, double_copy_window_ms: 800, max_text_length: 50000 },
    tts: {
      active_backend: "kokoro",
      active_profile_id: "default",
      profiles: [],
      preset: "kokoro-tts",
      command: "kokoro-tts",
      args_template: [],
      voice: "af_heart",
      openai: { api_key: "", model: "", voice: "" },
      elevenlabs: {
        api_key: "",
        voice_id: "",
        model_id: "",
        output_format: "mp3_44100_128",
        voice_stability: 0.5,
        voice_similarity_boost: 0.75
      },
      cartesia: { api_key: "", model_id: "", voice_id: "", output_format: "wav" },
      fish: { api_key: "", model: "", voice_id: "", format: "wav" },
      xai: { api_key: "", voice_id: "" },
      mistral: { api_key: "", voice_id: "" },
      google: { api_key: "", model: "", voice_name: "", output_format: "" },
      microsoft: { api_key: "", endpoint: "", model: "", voice_name: "", output_format: "" }
    },
    playback: {
      on_retrigger: "interrupt",
      volume: 100,
      streaming_enabled: true,
      duck: { enabled: false, level_percent: 20 }
    },
    hud: { enabled: false, position: "bottom-right", width: 300, height: 100, opacity: 0.85 },
    general: {
      launch_on_login: false,
      start_minimized: true,
      show_tray_icon: true,
      debug_mode: false,
      close_behavior: "minimize-to-tray",
      appearance: "system",
      locale: "en",
      experimental_enabled: false,
      update_checks_enabled: false,
      auto_save_profiles: false
    },
    output: {
      enabled: false,
      directory: "",
      filename_pattern: "test",
      format_config: { format: "wav", mp3_bitrate: 192, ogg_bitrate: 192, flac_compression: 5 }
    },
    sanitization: {
      enabled: true,
      markdown: {
        enabled: true,
        strip_headers: true,
        strip_code_blocks: true,
        strip_inline_code: false,
        strip_links: true,
        strip_bold_italic: true,
        strip_lists: true,
        strip_blockquotes: true
      },
      tts_normalization: { enabled: true }
    },
    pagination: { enabled: true, fragment_size: 600 },
    history: {
      enabled: false,
      storage_mode: "persistent",
      persistent_dir: null,
      auto_delete: { keep_latest: 15 },
      cleanup_orphaned_files: true
    },
    hotkey: { enabled: false, shortcut: "Win+Shift+A" },
    post_process: {
      enabled: false,
      provider_id: "test",
      providers: [{ id: "test", label: "Test", base_url: "", api_key: "", model: "" }],
      templates: [{ id: "concise", name: "Concise", class: "rewrite", prompt: "${output}" }],
      active_template_id: "concise",
      min_chars: 200,
      require_request_flag: false,
      timeout_secs: 5
    }
  };
}
