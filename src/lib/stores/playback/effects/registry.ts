// Single lookup point for active effect by id. New effects register here.

import type { Effect, EffectId } from "./types";
import { walkieTalkie } from "./walkie-talkie";
import { gameBoy } from "./game-boy";

interface EffectLookup {
  none: Effect | null;
  walkie_talkie: Effect | null;
  game_boy: Effect | null;
}

const EFFECTS: EffectLookup = {
  none: null,
  walkie_talkie: walkieTalkie,
  game_boy: gameBoy
};

export function getEffect(id: EffectId): Effect | null {
  return EFFECTS[id] ?? null;
}
