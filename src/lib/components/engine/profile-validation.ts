// Validation and normalization helpers for exported and imported VoiceProfiles.
// Used by ProfileExportDialog to ensure imported profiles satisfy the runtime
// contract before being committed to config, while preserving backwards
// compatibility with older profile schemas.

import type { EffectId, TtsEngine, VoiceProfile } from "#lib/types";

// Untrusted import JSON is validated field by field; prototype tags stand in
// for `typeof` and every check runs on the imported object directly.
// JSON.parse's own return type is the owner contract for imported payloads.
export type ParsedJson = ReturnType<typeof JSON.parse>;

const isObjectTag = (x: ParsedJson): boolean =>
  Object.prototype.toString.call(x) === "[object Object]";
const isStringTag = (x: ParsedJson): boolean =>
  Object.prototype.toString.call(x) === "[object String]";
const isBooleanTag = (x: ParsedJson): boolean =>
  Object.prototype.toString.call(x) === "[object Boolean]";
const isUndefinedTag = (x: ParsedJson): boolean =>
  Object.prototype.toString.call(x) === "[object Undefined]";

const ENGINES: TtsEngine[] = [
  "local",
  "http",
  "openai",
  "elevenlabs",
  "cartesia",
  "fish",
  "xai",
  "mistral",
  "google",
  "microsoft",
  "edge",
  "kitten",
  "qwen",
  "piper",
  "kokoro",
  "pocket"
];

const EFFECTS: EffectId[] = ["none", "walkie_talkie", "game_boy", "cyberpunk"];

/**
 * Validates that an untrusted parsed JSON payload conforms to the VoiceProfile shape.
 * `post_process` is optional for backwards compatibility with older exported
 * schemas; when present, its structure (enabled: boolean, active_template_id:
 * string | null | undefined) is strictly validated.
 */
export function isValidProfile(p: ParsedJson): p is VoiceProfile {
  if (!p || !isObjectTag(p)) return false;
  // SAFETY: the prototype tag above establishes a plain object; id/name/
  // engine/voice are string-tagged, engine/effect come from the registries,
  // and the optional post_process block is tag-checked field by field below.
  const o = p as VoiceProfile;
  if (
    !isStringTag(o.id) ||
    !isStringTag(o.name) ||
    !isStringTag(o.engine) ||
    !ENGINES.includes(o.engine) ||
    !isStringTag(o.voice) ||
    !Number.isFinite(o.speed) ||
    !Number.isFinite(o.pitch) ||
    !isObjectTag(o.effects) ||
    !EFFECTS.includes(o.effects.active_effect)
  ) {
    return false;
  }

  // Older exported profiles predate post_process; absence stays valid.
  if (isUndefinedTag(o.post_process)) return true;
  if (!isObjectTag(o.post_process)) return false;
  if (!isBooleanTag(o.post_process.enabled)) return false;
  // A missing or null active_template_id clears the override; anything else
  // must name a template.
  return (
    o.post_process.active_template_id === null ||
    isUndefinedTag(o.post_process.active_template_id) ||
    isStringTag(o.post_process.active_template_id)
  );
}

/**
 * Normalizes an imported VoiceProfile, ensuring `post_process` has default values
 * ({ enabled: false, active_template_id: null }) if absent or partially defined.
 */
export function normalizeImportedProfile(profile: VoiceProfile): VoiceProfile {
  const normalized: VoiceProfile = {
    ...profile,
    description: isStringTag(profile.description) ? profile.description : null,
    voice_label: isStringTag(profile.voice_label) ? profile.voice_label : null,
    engine_options: isObjectTag(profile.engine_options) ? profile.engine_options : {},
    post_process: { enabled: false, active_template_id: null }
  };
  if (isObjectTag(profile.post_process)) {
    const templateId = profile.post_process.active_template_id;
    normalized.post_process = {
      enabled: Boolean(profile.post_process.enabled),
      // SAFETY: the prototype tag above already established a string value.
      active_template_id: isStringTag(templateId) ? (templateId as string).trim() || null : null
    };
  }
  return normalized;
}
