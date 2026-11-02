import { describe, expect, it } from "vite-plus/test";
import { isValidProfile, normalizeImportedProfile, type ParsedJson } from "./profile-validation";
import type { VoiceProfile } from "#lib/types";

// Fixtures are untrusted parsed payloads, not typed profiles: the guard's
// boundary type accepts partial legacy shapes and the tests exercise
// field-by-field rejection on them directly.
function baseValidProfile(): ParsedJson {
  return {
    id: "test-profile",
    name: "Test Profile",
    engine: "kokoro",
    voice: "af_heart",
    speed: 1.0,
    pitch: 1.0,
    effects: {
      enabled: false,
      active_effect: "none"
    }
  };
}

describe("isValidProfile", () => {
  it("accepts a valid profile without post_process (legacy format)", () => {
    const valid = baseValidProfile();
    expect(isValidProfile(valid)).toBe(true);
  });

  it("accepts a valid profile with complete post_process", () => {
    const valid = {
      ...baseValidProfile(),
      post_process: {
        enabled: true,
        active_template_id: "concise"
      }
    };
    expect(isValidProfile(valid)).toBe(true);
  });

  it("accepts a valid profile with null active_template_id", () => {
    const valid = {
      ...baseValidProfile(),
      post_process: {
        enabled: false,
        active_template_id: null
      }
    };
    expect(isValidProfile(valid)).toBe(true);
  });

  it("accepts a valid profile with undefined active_template_id", () => {
    const valid = {
      ...baseValidProfile(),
      post_process: {
        enabled: false
      }
    };
    expect(isValidProfile(valid)).toBe(true);
  });

  it("rejects non-object or null root payloads", () => {
    expect(isValidProfile(null)).toBe(false);
    expect(isValidProfile(undefined)).toBe(false);
    expect(isValidProfile("string")).toBe(false);
    expect(isValidProfile(123)).toBe(false);
    expect(isValidProfile([])).toBe(false);
  });

  it("rejects profiles missing core fields", () => {
    const missingId = baseValidProfile();
    delete missingId.id;
    expect(isValidProfile(missingId)).toBe(false);

    const missingEngine = baseValidProfile();
    delete missingEngine.engine;
    expect(isValidProfile(missingEngine)).toBe(false);

    const missingEffects = baseValidProfile();
    delete missingEffects.effects;
    expect(isValidProfile(missingEffects)).toBe(false);
  });

  it("rejects invalid engines or effect ids", () => {
    expect(isValidProfile({ ...baseValidProfile(), engine: "unknown-engine" })).toBe(false);
    expect(
      isValidProfile({
        ...baseValidProfile(),
        effects: { enabled: true, active_effect: "laser-eyes" }
      })
    ).toBe(false);
  });

  it("rejects non-object post_process", () => {
    expect(isValidProfile({ ...baseValidProfile(), post_process: "enabled" })).toBe(false);
    expect(isValidProfile({ ...baseValidProfile(), post_process: true })).toBe(false);
    expect(isValidProfile({ ...baseValidProfile(), post_process: 123 })).toBe(false);
    expect(isValidProfile({ ...baseValidProfile(), post_process: null })).toBe(false);
    expect(isValidProfile({ ...baseValidProfile(), post_process: [] })).toBe(false);
  });

  it("rejects post_process with non-boolean enabled field", () => {
    expect(
      isValidProfile({
        ...baseValidProfile(),
        post_process: { enabled: "true", active_template_id: null }
      })
    ).toBe(false);
    expect(
      isValidProfile({
        ...baseValidProfile(),
        post_process: { enabled: 1, active_template_id: null }
      })
    ).toBe(false);
  });

  it("rejects post_process with non-string and non-null active_template_id", () => {
    expect(
      isValidProfile({
        ...baseValidProfile(),
        post_process: { enabled: true, active_template_id: 123 }
      })
    ).toBe(false);
    expect(
      isValidProfile({
        ...baseValidProfile(),
        post_process: { enabled: true, active_template_id: {} }
      })
    ).toBe(false);
    expect(
      isValidProfile({
        ...baseValidProfile(),
        post_process: { enabled: true, active_template_id: false }
      })
    ).toBe(false);
  });
});

describe("normalizeImportedProfile", () => {
  // A complete, current-schema profile. Legacy variants below strip fields via
  // a JSON round trip, mirroring the dialog where payloads arrive from
  // JSON.parse and may predate fields the current schema requires.
  function typedProfile(): VoiceProfile {
    return {
      id: "test-profile",
      name: "Test Profile",
      description: null,
      engine: "kokoro",
      voice: "af_heart",
      voice_label: null,
      speed: 1.0,
      pitch: 1.0,
      effects: {
        enabled: false,
        active_effect: "none"
      },
      post_process: {
        enabled: false,
        active_template_id: null
      },
      engine_options: {}
    };
  }

  it("supplies default post_process when missing (legacy profile)", () => {
    const legacy = JSON.parse(JSON.stringify(typedProfile()));
    delete legacy.post_process;
    expect(isValidProfile(legacy)).toBe(true);
    const normalized = normalizeImportedProfile(legacy);
    expect(normalized.post_process).toEqual({
      enabled: false,
      active_template_id: null
    });
  });

  it("preserves valid post_process settings", () => {
    const normalized = normalizeImportedProfile({
      ...typedProfile(),
      post_process: { enabled: true, active_template_id: "custom-1" }
    });
    expect(normalized.post_process).toEqual({
      enabled: true,
      active_template_id: "custom-1"
    });
  });

  it("normalizes empty or whitespace active_template_id to null", () => {
    const normalized = normalizeImportedProfile({
      ...typedProfile(),
      post_process: { enabled: true, active_template_id: "   " }
    });
    expect(normalized.post_process.active_template_id).toBeNull();
  });

  it("normalizes undefined active_template_id to null", () => {
    const raw = JSON.parse(JSON.stringify(typedProfile()));
    raw.post_process = { enabled: true };
    expect(isValidProfile(raw)).toBe(true);
    const normalized = normalizeImportedProfile(raw);
    expect(normalized.post_process.active_template_id).toBeNull();
  });
});
