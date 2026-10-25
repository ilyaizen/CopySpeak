// Pure selection/validation logic for the post-process prompt-template
// library. Mirrors the backend's invariants (builtins are non-deletable,
// customs default to rewrite) so the settings UI and the Home quick surface
// share one implementation. The backend normalize() stays the ultimate
// authority on load; these helpers only shape frontend edits.

import type { PostProcessConfig, PostProcessTemplate } from "#lib/types";

export const BUILTIN_TEMPLATE_IDS = ["dramatize", "concise"] as const;

export function isBuiltinTemplate(id: string): boolean {
  return (BUILTIN_TEMPLATE_IDS as readonly string[]).includes(id);
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
