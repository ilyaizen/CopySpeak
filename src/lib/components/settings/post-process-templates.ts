// Pure selection/validation logic for the post-process prompt-template
// library. Mirrors the backend's invariants (builtins are non-deletable,
// customs default to rewrite) so the settings UI and the Home quick surface
// share one implementation. The backend normalize() stays the ultimate
// authority on load; these helpers only shape frontend edits.

import type { PostProcessConfig, PostProcessTemplate, VoiceProfile } from "#lib/types";

export const BUILTIN_TEMPLATE_IDS = ["dramatize", "concise"] as const;

export function isBuiltinTemplate(id: string): boolean {
  return BUILTIN_TEMPLATE_IDS.some((builtin) => builtin === id);
}

/** The active template, falling back to the first entry like the backend. */
export function activeTemplate(config: PostProcessConfig): PostProcessTemplate | undefined {
  return config.templates.find((t) => t.id === config.active_template_id) ?? config.templates[0];
}

/** Dropdown options for the template library, in display order. */
export function templateOptions(templates: PostProcessTemplate[]): Array<{
  value: string;
  label: string;
}> {
  return templates.map((t) => ({ value: t.id, label: t.name }));
}

/** Whether the template needs a tag-capable engine: preserve-class only. */
export function needsEmoteSupport(template: PostProcessTemplate | undefined): boolean {
  return template?.class === "preserve";
}

/**
 * Whether the Post-process switch must be disabled: only when the ACTIVE
 * template is preserve-class and the engine cannot read emote tags. A
 * rewrite template (Concise) runs on any engine, so the switch stays usable.
 */
export function switchDisabledFor(
  template: PostProcessTemplate | undefined,
  emoteSupported: boolean
): boolean {
  return needsEmoteSupport(template) && !emoteSupported;
}

/** A new custom template: rewrite class by design, unique id, draft name. */
export function createCustomTemplate(templates: PostProcessTemplate[]): PostProcessTemplate {
  const id = `custom-${Date.now().toString(36)}-${templates.length}`;
  return { id, name: "New template", class: "rewrite", prompt: "" };
}

/** The id the active selection falls back to after a deletion. */
export function fallbackTemplateId(templates: PostProcessTemplate[]): string {
  return templates.find((t) => t.id === "dramatize")?.id ?? templates[0]?.id ?? "";
}

/**
 * Resolves the effective template for a profile, accounting for profile-specific
 * overrides and dangling template references.
 *
 * If a profile specifies an active_template_id that exists in `config.templates`,
 * that template is returned.
 * If the profile does not specify an override (null or empty), or references a
 * dangling/deleted template id, it falls back to the globally configured active
 * template (and then to the first template if the global selection is also dangling).
 */
export function resolveProfileTemplate(
  config: PostProcessConfig,
  profileTemplateId?: string | null
): PostProcessTemplate | undefined {
  if (profileTemplateId) {
    const matched = config.templates.find((t) => t.id === profileTemplateId);
    if (matched) return matched;
  }
  return activeTemplate(config);
}

/**
 * Clears dangling active_template_id references in profiles when a template is deleted,
 * resetting them to null so they cleanly fall back to the global configured template.
 */
export function clearDeletedTemplateFromProfiles(
  profiles: VoiceProfile[],
  deletedTemplateId: string
): void {
  for (const profile of profiles) {
    if (profile.post_process && profile.post_process.active_template_id === deletedTemplateId) {
      profile.post_process.active_template_id = null;
    }
  }
}
