export type ValidationError =
  | {
      type: "OpacityOutOfRange";
      value: number;
      min: number;
      max: number;
    }
  | {
      type: "CommandEmpty";
    }
  | {
      type: "ArgsTemplateMissingPlaceholder";
      placeholder: string;
    }
  | {
      type: "DoubleCopyWindowTooSmall";
      value: number;
      min: number;
    }
  | {
      type: "MaxTextLengthTooSmall";
      value: number;
      min: number;
    };

export type ValidationResult = ValidationError[];

export type SupportedLocale = "en" | "es" | "ar" | "he";

export type HudPresetPosition =
  | "top-left"
  | "top-center"
  | "top-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right";

export type HudPosition = HudPresetPosition;

export type RetriggerMode = "interrupt" | "queue";

export interface TriggerConfig {
  listen_enabled: boolean;
  double_copy_window_ms: number;
  max_text_length: number;
}

export type TtsEngine =
  | "local"
  | "http"
  | "openai"
  | "elevenlabs"
  | "cartesia"
  | "google"
  | "microsoft"
  | "edge";

export interface OpenAIConfig {
  api_key: string;
  model: string;
  voice: string;
}

export type ElevenLabsOutputFormat =
  | "mp3_44100_128"
  | "mp3_44100_192"
  | "mp3_44100_32"
  | "mp3_22050_32"
  | "pcm_44100"
  | "pcm_22050"
  | "pcm_16000"
  | "ogg_vorbis_44100"
  | "ogg_vorbis_22050"
  | "flac_44100"
  | "mulaw_8000";

export interface ElevenLabsConfig {
  api_key: string;
  voice_id: string;
  voice_name?: string;
  model_id: string;
  output_format: ElevenLabsOutputFormat;
  voice_stability: number;
  voice_similarity_boost: number;
  voice_style?: number;
  use_speaker_boost?: boolean;
  use_manual_voice_id?: boolean;
}

export interface CartesiaConfig {
  api_key: string;
  model_id: string;
  voice_id: string;
  voice_name?: string;
  output_format: string;
  use_manual_voice_id?: boolean;
}

export interface TtsConfig {
  active_backend: TtsEngine;
  active_profile_id: string;
  profiles: VoiceProfile[];
  preset: string;
  command: string;
  args_template: string[];
  voice: string;
  openai: OpenAIConfig;
  elevenlabs: ElevenLabsConfig;
  cartesia: CartesiaConfig;
  edge?: EdgeTtsConfig;
  google: GoogleTtsConfig;
  microsoft: MicrosoftTtsConfig;
  http?: HttpTtsConfig;
}

// ── Edge-TTS ──────────────────────────────────────────────────────────────────

export interface EdgeTtsConfig {
  voice: string;
}

// ── Google Gemini TTS ────────────────────────────────────────────────────────

export interface GoogleTtsConfig {
  api_key: string;
  model: string;
  voice_name: string;
  output_format: string;
}

// ── Microsoft AI ─────────────────────────────────────────────────────────────

export interface MicrosoftTtsConfig {
  api_key: string;
  endpoint: string;
  model: string;
  voice_name: string;
  output_format: string;
}

// ── HTTP TTS ────────────────────────────────────────────────────────────────

export interface HttpTtsConfig {
  profile_id: string;
  url_template: string;
  method: string;
  headers: [string, string][];
  body_template: string | null;
  voice: string;
  response_format: string;
  timeout_secs: number;
}

// ── Voice profiles ────────────────────────────────────────────────────────────

export interface ProfileEffects {
  enabled: boolean;
  active_effect: EffectId;
}

export interface VoiceProfile {
  id: string;
  name: string;
  description: string | null;
  engine: TtsEngine;
  voice: string;
  voice_label: string | null;
  speed: number;
  pitch: number;
  effects: ProfileEffects;
  engine_options: Record<string, unknown>;
}

// ── Engine catalog (returned by list_tts_engines IPC) ────────────────────────

export interface EngineOptionDescriptor {
  key: string;
  label: string;
  kind: string;
  help: string;
  default_value: unknown;
}

export interface VoiceCatalogEntry {
  id: string;
  label: string;
  language: string | null;
  description: string | null;
  preview_url: string | null;
}

export interface EngineCatalogEntry {
  engine: string;
  label: string;
  description: string;
  docs_url: string;
  supports_voice_refresh: boolean;
  supports_pitch: boolean;
  supports_bracket_emotes: boolean;
  options: EngineOptionDescriptor[];
  voices: VoiceCatalogEntry[];
}

export type PostProcessingProvider =
  | "groq"
  | "openai"
  | "anthropic"
  | "gemini"
  | "openrouter"
  | "ollama"
  | "xai"
  | "aws"
  | "cerebras"
  | "custom";

export interface PostProcessingPromptPreset {
  label: string;
  prompt: string;
}

export interface LlmProviderConfig {
  api_key: string;
  model: string;
  endpoint: string;
}

export interface PostProcessingConfig {
  enabled: boolean;
  provider: PostProcessingProvider;
  prompt: string;
  selected_prompt_label: string;
  prompt_presets: PostProcessingPromptPreset[];
  groq: LlmProviderConfig;
  openai: LlmProviderConfig;
  anthropic: LlmProviderConfig;
  gemini: LlmProviderConfig;
  openrouter: LlmProviderConfig;
  ollama: LlmProviderConfig;
  xai: LlmProviderConfig;
  aws: LlmProviderConfig;
  cerebras: LlmProviderConfig;
  custom: LlmProviderConfig;
}

export interface PlaybackConfig {
  on_retrigger: RetriggerMode;
  volume: number;
}

export type HudThemePreset = "dark" | "light" | "custom";

export interface HudThemeConfig {
  preset: HudThemePreset;
  waveform_color: string;
  waveform_active_color: string;
  background_color: string;
  border_radius: number;
  animation_speed: number;
}

export interface HudConfig {
  enabled: boolean;
  position: HudPosition;
  width: number;
  height: number;
  opacity: number;
  theme?: HudThemeConfig; // optional — not exposed in settings UI
}

export type CloseBehavior = "minimize-to-tray" | "exit";

export type AppearanceMode = "system" | "light" | "dark";

export interface GeneralConfig {
  start_with_windows: boolean;
  start_minimized: boolean;
  debug_mode: boolean;
  close_behavior: CloseBehavior;
  appearance: AppearanceMode;
  update_checks_enabled?: boolean;
  locale: SupportedLocale;
}

export type AudioFormat = "wav" | "mp3" | "ogg" | "flac";

export interface FormatConfig {
  format: AudioFormat;
  mp3_bitrate: number;
  ogg_bitrate: number;
  flac_compression: number;
}

export interface OutputConfig {
  enabled: boolean;
  directory: string;
  filename_pattern: string;
  format_config: FormatConfig;
}

export interface MarkdownSanitizationConfig {
  enabled: boolean;
  strip_headers: boolean;
  strip_code_blocks: boolean;
  strip_inline_code: boolean;
  strip_links: boolean;
  strip_bold_italic: boolean;
  strip_lists: boolean;
  strip_blockquotes: boolean;
}

export interface TtsNormalizationConfig {
  enabled: boolean;
}

export interface SanitizationConfig {
  enabled: boolean;
  markdown: MarkdownSanitizationConfig;
  tts_normalization: TtsNormalizationConfig;
}

export interface PaginationConfig {
  enabled: boolean;
  fragment_size: number;
}

export type StorageMode = "temp" | "persistent";

export type AutoDeleteMode = { keep_latest: number } | "never" | { after_days: number };

export interface HistoryConfig {
  enabled: boolean;
  storage_mode: StorageMode;
  persistent_dir: string | null;
  auto_delete: AutoDeleteMode;
  cleanup_orphaned_files: boolean;
}

export interface HotkeyConfig {
  enabled: boolean;
  shortcut: string;
}

export type EffectId = "none" | "walkie_talkie" | "game_boy";

export interface PostProcessConfig {
  enabled: boolean;
  api_key: string;
  model: string;
  prompt: string;
}

export interface AppConfig {
  trigger: TriggerConfig;
  tts: TtsConfig;
  playback: PlaybackConfig;
  hud: HudConfig;
  general: GeneralConfig;
  output: OutputConfig;
  sanitization: SanitizationConfig;
  pagination: PaginationConfig;
  history: HistoryConfig;
  hotkey: HotkeyConfig;
  post_process: PostProcessConfig;
}

// History types for tracking text-to-speech operations
export type HistoryEventType = "speak" | "stop" | "pause" | "resume" | "speed_change" | "error";

export interface HistoryEventMetadata {
  [key: string]: string | number | boolean | null;
}

export interface HistoryEvent {
  id: string;
  timestamp: number; // Unix timestamp in milliseconds
  event_type: HistoryEventType;
  text: string;
  duration_ms?: number;
  output_path?: string;
  tts_engine?: TtsEngine;
  voice?: string;
  speed?: number;
  metadata?: HistoryEventMetadata;
  success: boolean;
  error_message?: string;
}

export interface HistoryItemStatus {
  current: number;
  total: number;
  percentage: number;
}

export interface HistoryItem {
  id: string;
  timestamp: number; // Unix timestamp in milliseconds
  text: string;
  text_length: number;
  tts_engine: TtsEngine;
  voice: string;
  speed: number;
  output_format?: AudioFormat;
  output_path?: string;
  duration_ms?: number;
  batch_id?: string;
  app_name?: string;
  source?: string;
  filters_applied?: string[];
  success: boolean;
  error_message?: string;
  attempts: number;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

export interface HistoryFilters {
  search_text?: string;
  tts_engine?: TtsEngine;
  voice?: string;
  date_from?: number;
  date_to?: number;
  success_only?: boolean;
  failed_only?: boolean;
  tags?: string[];
  app_name?: string;
}

export interface HistorySortOptions {
  sort_by: "timestamp" | "text" | "duration" | "engine";
  order: "ascending" | "descending";
}

export interface HistoryPaginationOptions {
  limit: number;
  offset: number;
}

export interface HistoryQueryResult {
  items: HistoryItem[];
  total_count: number;
  limit: number;
  offset: number;
}

export interface HistoryStatistics {
  total_items: number;
  total_duration_ms: number;
  successful_items: number;
  failed_items: number;
  success_rate: number;
  by_engine: Record<TtsEngine, number>;
  by_format: Record<AudioFormat, number>;
  by_hour: Record<number, number>; // hour (0-23) -> count
  by_day: Record<string, number>; // date string (YYYY-MM-DD) -> count
  most_used_voice: string | null;
  average_text_length: number;
  average_duration_ms: number;
}

export interface HistoryExportOptions {
  format: "json" | "csv";
  include_metadata: boolean;
  date_from?: number;
  date_to?: number;
  filters?: HistoryFilters;
}

export interface HistoryExportResult {
  export_id: string;
  created_at: number;
  file_path: string;
  format: "json" | "csv";
  item_count: number;
  file_size_bytes: number;
}

export interface HistoryDeletionResult {
  deleted_count: number;
  freed_space_bytes: number;
}

export interface HistoryState {
  items: HistoryItem[];
  events: HistoryEvent[];
  statistics: HistoryStatistics;
  config: HistoryConfig;
  is_loading: boolean;
  error: string | null;
  last_updated: number;
}
