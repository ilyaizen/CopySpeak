import { describe, expect, it, vi, beforeEach } from "vite-plus/test";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import ProfileManager from "./profile-manager.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";

// A catalog entry with important and advanced options, so the test can verify
// the model and GPU stay visible while one disclosure groups the rest.
const { CATALOG } = vi.hoisted(() => ({
  CATALOG: [
    {
      engine: "openai",
      label: "OpenAI",
      description: "Cloud TTS",
      docs_url: "https://platform.openai.com/docs/guides/text-to-speech",
      supports_voice_refresh: false,
      supports_pitch: false,
      supports_captions: true,
      supports_bracket_emotes: false,
      options: [
        {
          key: "model",
          label: "Model",
          kind: "select",
          help: "Speech model.",
          default_value: "tts-1",
          choices: ["tts-1", "gpt-4o-mini-tts"]
        },
        {
          key: "cuda",
          label: "GPU acceleration",
          kind: "boolean",
          help: "Run on GPU.",
          default_value: false
        },
        {
          key: "response_format",
          label: "Response Format",
          kind: "select",
          help: "Audio container.",
          default_value: "mp3",
          choices: ["mp3", "wav"]
        },
        { key: "speed", label: "Speed", kind: "number", help: "Speech pace.", default_value: 1.0 },
        {
          key: "instructions",
          label: "Instructions",
          kind: "text",
          help: "Voice style.",
          default_value: ""
        }
      ],
      voices: []
    }
  ]
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "list_tts_engines") return CATALOG;
    if (cmd === "has_engine_credentials") return true;
    return undefined;
  })
}));

vi.mock("svelte-sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() }
}));

async function renderOpenAIProfile() {
  const config = testConfig();
  config.tts.active_profile_id = "p1";
  config.tts.profiles = [
    {
      id: "p1",
      name: "Test Profile",
      description: null,
      engine: "openai",
      voice: "",
      voice_label: null,
      speed: 1.0,
      pitch: 1.0,
      effects: { enabled: false, active_effect: "none" },
      post_process: { enabled: false, active_template_id: null },
      engine_options: {}
    }
  ];
  const view = render(ProfileManager, { localConfig: config });
  // The catalog arrives through an async invoke; wait for the option rows.
  await waitFor(() => expect(view.getByRole("button", { name: "Advanced" })).toBeTruthy());
  return view;
}

// bits-ui keeps a closed Collapsible's content mounted behind a `hidden`
// attribute, so "collapsed" means hidden, not absent.
function visibleSelects(
  view: ReturnType<typeof render<typeof ProfileManager>>
): HTMLSelectElement[] {
  return [...view.container.querySelectorAll("select")].filter(
    (el) => el.closest("[hidden]") === null
  );
}

describe("profile-manager advanced engine options", () => {
  beforeEach(async () => {
    await waitForI18nReady();
  });

  it("keeps model and GPU visible and groups other options in one disclosure", async () => {
    const view = await renderOpenAIProfile();

    // Profile, engine, model and GPU selectors stay visible. The advanced
    // Response Format selector remains hidden until its one disclosure opens.
    const visible = visibleSelects(view);
    expect(visible).toHaveLength(4);
    const modelSelect = visible.find((s) =>
      [...s.options].some((o) => o.value === "gpt-4o-mini-tts")
    );
    if (!modelSelect) throw new Error("Model select is not visible");
    expect(modelSelect.value).toBe("tts-1");

    expect(visible.some((s) => [...s.options].some((o) => o.value === "false"))).toBe(true);
    const speedInput = view.container.querySelector('input[type="number"]');
    if (!(speedInput instanceof HTMLInputElement)) throw new Error("Speed input missing");
    expect(speedInput.closest("[hidden]")).not.toBeNull();
    const advanced = view.getByRole("button", { name: "Advanced" });
    expect(advanced.getAttribute("aria-expanded")).toBe("false");
    expect(view.queryAllByRole("button", { name: "Speed" })).toHaveLength(0);
    expect(view.queryAllByRole("button", { name: "Response Format" })).toHaveLength(0);
    expect(view.queryAllByRole("button", { name: "Instructions" })).toHaveLength(0);

    view.unmount();
  });

  it("expands the advanced options together", async () => {
    const view = await renderOpenAIProfile();

    await fireEvent.click(view.getByRole("button", { name: "Advanced" }));
    const speedInput = view.container.querySelector('input[type="number"]');
    if (!(speedInput instanceof HTMLInputElement)) throw new Error("Speed input did not appear");
    expect(speedInput.value).toBe("1");
    expect(speedInput.closest("[hidden]")).toBeNull();
    expect(view.getByRole("button", { name: "Advanced" }).getAttribute("aria-expanded")).toBe(
      "true"
    );

    expect(visibleSelects(view)).toHaveLength(5);

    view.unmount();
  });
});
