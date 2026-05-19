// Single lookup point for active effect by id. New effects register here.

import type { Effect, EffectId } from "./types";
import { walkieTalkie } from "./walkie-talkie";
import { gameBoy } from "./game-boy";

const EFFECTS: Record<EffectId, Effect | null> = {
  none: null,
  walkie_talkie: walkieTalkie,
  game_boy: gameBoy
};

export function getEffect(id: EffectId): Effect | null {
  return EFFECTS[id] ?? null;
}
