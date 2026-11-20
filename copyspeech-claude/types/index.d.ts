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
      /** Per AssistantMessage requestId: the spoken word's [start, end) in that row's markdown, or null. */
      spoken: StateFamily<[number, number] | null>;
    };
  }
}
