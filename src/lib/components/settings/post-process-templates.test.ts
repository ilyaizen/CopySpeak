import { describe, expect, it } from "vite-plus/test";
import {
  activeTemplate,
  createCustomTemplate,
  fallbackTemplateId,
  isBuiltinTemplate,
  needsEmoteSupport,
  switchDisabledFor,
  templateOptions
} from "./post-process-templates";
import type { PostProcessConfig, PostProcessTemplate } from "#lib/types";

function template(id: string, templateClass: PostProcessTemplate["class"]): PostProcessTemplate {
  return { id, name: id, class: templateClass, prompt: `${id} ${"${output}"}` };
}

function config(templates: PostProcessTemplate[], activeId: string): PostProcessConfig {
  return {
    enabled: false,
    provider_id: "groq",
    providers: [],
    templates,
    active_template_id: activeId,
    min_chars: 0,
    require_request_flag: false,
    timeout_secs: 5
  } satisfies PostProcessConfig;
}

describe("activeTemplate", () => {
  it("returns the entry matching active_template_id", () => {
    const templates = [template("dramatize", "preserve"), template("concise", "rewrite")];
    expect(activeTemplate(config(templates, "concise"))?.id).toBe("concise");
  });

  it("falls back to the first entry when the id dangles", () => {
    const templates = [template("dramatize", "preserve"), template("concise", "rewrite")];
    expect(activeTemplate(config(templates, "deleted"))?.id).toBe("dramatize");
  });

  it("returns undefined for an empty library", () => {
    expect(activeTemplate(config([], "dramatize"))).toBeUndefined();
  });
});

describe("switchDisabledFor", () => {
  it("disables preserve-class templates on engines without emote tags", () => {
    const dramatize = template("dramatize", "preserve");
    expect(switchDisabledFor(dramatize, false)).toBe(true);
    expect(switchDisabledFor(dramatize, true)).toBe(false);
  });

  it("keeps rewrite templates usable on any engine", () => {
    const concise = template("concise", "rewrite");
    expect(switchDisabledFor(concise, false)).toBe(false);
  });

  it("treats a missing template as usable", () => {
    expect(switchDisabledFor(undefined, false)).toBe(false);
  });

  it("mirrors needsEmoteSupport for the preserve class only", () => {
    expect(needsEmoteSupport(template("dramatize", "preserve"))).toBe(true);
    expect(needsEmoteSupport(template("concise", "rewrite"))).toBe(false);
    expect(needsEmoteSupport(undefined)).toBe(false);
  });
});

describe("isBuiltinTemplate", () => {
  it("accepts exactly the builtin ids", () => {
    expect(isBuiltinTemplate("dramatize")).toBe(true);
    expect(isBuiltinTemplate("concise")).toBe(true);
    expect(isBuiltinTemplate("custom-abc")).toBe(false);
  });
});

describe("createCustomTemplate", () => {
  it("defaults to the rewrite class with a draft prompt", () => {
    const created = createCustomTemplate([]);
    expect(created.class).toBe("rewrite");
    expect(created.prompt).toBe("");
    expect(created.name).toBe("New template");
  });

  it("produces unique ids across calls", () => {
    const first = createCustomTemplate([]);
    const second = createCustomTemplate([first]);
    expect(first.id).not.toBe(second.id);
    expect(isBuiltinTemplate(first.id)).toBe(false);
  });
});

describe("templateOptions", () => {
  it("maps templates to select options in display order", () => {
    const templates = [template("dramatize", "preserve"), template("concise", "rewrite")];
    expect(templateOptions(templates)).toEqual([
      { value: "dramatize", label: "dramatize" },
      { value: "concise", label: "concise" }
    ]);
  });
});

describe("fallbackTemplateId", () => {
  it("prefers dramatize, then the first entry, then empty", () => {
    const templates = [template("dramatize", "preserve"), template("concise", "rewrite")];
    expect(fallbackTemplateId(templates)).toBe("dramatize");
    expect(fallbackTemplateId([template("concise", "rewrite")])).toBe("concise");
    expect(fallbackTemplateId([])).toBe("");
  });
});
