/** The reading this mod started, or null while idle. */
export type Reading = {
  /** CopySpeech request id. */
  id: string;
  /** AssistantMessage requestId that holds the spoken word: the first of `rows` until a word matches. */
  row: string;
  /** Every AssistantMessage requestId the reading covers, in order. Values stored before rows existed lack it. */
  rows?: string[];
  paused: boolean;
  /** Spoken word's [start, end) in `row`'s markdown, or null. */
  mark: [number, number] | null;
};

declare module "claude-code" {
  interface PluginState {
    copyspeech: { reading: Reading | null };
  }
}
