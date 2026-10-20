import type { EffectId, ProfileEffects } from "#lib/types";

/** The part of AppConfig that decides which effect plays. Structural, so AppConfig satisfies it. */
export interface EffectGateConfig {
  general: { experimental_enabled?: boolean };
  tts: {
    active_profile_id: string;
    profiles: { id: string; effects: ProfileEffects }[];
  };
}

/**
 * The effect playback applies. Experimental off forces "none" at the source;
 * the profile's saved effect stays in config for when Experimental is back on.
 */
export function effectiveEffect(config: EffectGateConfig): EffectId {
  if (config.general.experimental_enabled !== true) return "none";
  const profile = config.tts.profiles.find((p) => p.id === config.tts.active_profile_id);
  return profile?.effects.enabled ? profile.effects.active_effect : "none";
}
