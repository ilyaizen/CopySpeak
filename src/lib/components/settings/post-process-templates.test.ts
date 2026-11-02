import { describe, expect, it } from "vite-plus/test";
import {
  activeTemplate,
  clearDeletedTemplateFromProfiles,
  createCustomTemplate,
  fallbackTemplateId,
  isBuiltinTemplate,
  needsEmoteSupport,
  resolveProfileTemplate,
  switchDisabledFor,
  templateOptions
} from "./post-process-templates";
import type { PostProcessConfig, PostProcessTemplate, VoiceProfile } from "#lib/types";

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

describe("resolveProfileTemplate", () => {
  const templates = [
    template("dramatize", "preserve"),
    template("concise", "rewrite"),
    template("custom-1", "rewrite")
  ];
  const cfg = config(templates, "concise");

  it("returns profile template when it exists in config templates", () => {
    expect(resolveProfileTemplate(cfg, "custom-1")?.id).toBe("custom-1");
  });

  it("falls back to the global active configured template when profile override is null or empty", () => {
    expect(resolveProfileTemplate(cfg, null)?.id).toBe("concise");
    expect(resolveProfileTemplate(cfg, "")?.id).toBe("concise");
    expect(resolveProfileTemplate(cfg, undefined)?.id).toBe("concise");
  });

  it("falls back to global active configured template when profile template id dangles", () => {
    // When profile references a nonexistent/deleted template, resolve to the global active template (concise),
    // NOT directly to templates[0] (dramatize).
    expect(resolveProfileTemplate(cfg, "nonexistent-template")?.id).toBe("concise");
  });

  it("falls back to first template when both profile override and global active id dangle", () => {
    const danglingGlobalCfg = config(templates, "dangling-global");
    expect(resolveProfileTemplate(danglingGlobalCfg, "nonexistent-profile-tmpl")?.id).toBe(
      "dramatize"
    );
  });
});

describe("clearDeletedTemplateFromProfiles", () => {
  it("resets active_template_id to null for profiles referencing the deleted template", () => {
    const profiles: VoiceProfile[] = [
      {
        id: "p1",
        name: "P1",
        description: null,
        voice_label: null,
        engine_options: {},
        engine: "kokoro",
        voice: "v1",
        speed: 1,
        pitch: 1,
        effects: { enabled: false, active_effect: "none" },
        post_process: { enabled: true, active_template_id: "victim-id" }
      },
      {
        id: "p2",
        name: "P2",
        description: null,
        voice_label: null,
        engine_options: {},
        engine: "kokoro",
        voice: "v2",
        speed: 1,
        pitch: 1,
        effects: { enabled: false, active_effect: "none" },
        post_process: { enabled: true, active_template_id: "keep-id" }
      },
      {
        id: "p3",
        name: "P3",
        description: null,
        voice_label: null,
        engine_options: {},
        engine: "kokoro",
        voice: "v3",
        speed: 1,
        pitch: 1,
        effects: { enabled: false, active_effect: "none" },
        post_process: { enabled: false, active_template_id: null }
      }
    ];

    clearDeletedTemplateFromProfiles(profiles, "victim-id");

    expect(profiles[0].post_process?.active_template_id).toBeNull();
    expect(profiles[1].post_process?.active_template_id).toBe("keep-id");
    expect(profiles[2].post_process?.active_template_id).toBeNull();
  });
});
