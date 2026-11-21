/** The reading this mod started, or null while idle. Changes only at start, pause, resume and stop. */
export type Reading = {
  /** CopySpeech request id. */
  id: string;
  /** Every AssistantMessage requestId the reading covers, in order. */
  rows: string[];
  paused: boolean;
};

declare module "claude-code" {
  interface PluginState {
    copyspeech: {
      reading: Reading | null;
      /** The spoken word: its AssistantMessage requestId and [start, end) in that row's markdown, or null. */
      spoken: { row: string; at: [number, number] } | null;
      /** The replies the captions pane shows: the latest reading's AssistantMessage requestIds and markdown. */
      history: { row: string; md: string }[];
      /** Whether the captions pane's settings are expanded. */
      settingsOpen: boolean;
    };
  }
}
