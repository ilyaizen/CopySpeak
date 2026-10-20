import { describe, expect, test } from "vite-plus/test";
import { effectiveEffect, type EffectGateConfig } from "#lib/effects-gate";

function config(experimental: boolean | undefined, enabled: boolean): EffectGateConfig {
  return {
    general: { experimental_enabled: experimental },
    tts: {
      active_profile_id: "p1",
      profiles: [{ id: "p1", effects: { enabled, active_effect: "walkie_talkie" } }]
    }
  };
}

describe("effectiveEffect", () => {
  test("experimental on plays the active profile's effect", () => {
    expect(effectiveEffect(config(true, true))).toBe("walkie_talkie");
  });

  test("experimental off plays nothing even when the profile enables an effect", () => {
    expect(effectiveEffect(config(false, true))).toBe("none");
  });

  test("a config saved before the flag existed counts as experimental off", () => {
    expect(effectiveEffect(config(undefined, true))).toBe("none");
  });

  test("a disabled profile effect plays nothing", () => {
    expect(effectiveEffect(config(true, false))).toBe("none");
  });

  test("a missing active profile plays nothing", () => {
    const c = config(true, true);
    c.tts.active_profile_id = "gone";
    expect(effectiveEffect(c)).toBe("none");
  });
});
