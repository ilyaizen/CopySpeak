// Effect abstraction: every effect transforms one decoded AudioBuffer into
// another, using an AudioContext supplied by the pipeline (so effects can
// decode bundled samples like clicks without owning their own context).

import type { EffectId } from "$lib/types";

export type { EffectId };

export interface Effect {
  readonly id: EffectId;
  process(input: AudioBuffer, ctx: AudioContext): Promise<AudioBuffer>;
}
