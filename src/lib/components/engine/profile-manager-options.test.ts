import { describe, expect, it, vi, beforeEach } from "vite-plus/test";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import ProfileManager from "./profile-manager.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";

// A catalog entry exercising every option kind beside the model, so the test
// can watch the non-model options stay collapsed until their row is opened.
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
  await waitFor(() => expect(view.getByRole("button", { name: "Speed" })).toBeTruthy());
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

describe("profile-manager engine option collapse", () => {
  beforeEach(async () => {
    await waitForI18nReady();
  });

  it("keeps the model expanded and every other engine option collapsed", async () => {
    const view = await renderOpenAIProfile();

    // The profile selector, the engine selector and the model are the only
    // visible selects; the collapsed Response Format select is hidden.
    const visible = visibleSelects(view);
    expect(visible).toHaveLength(3);
    const modelSelect = visible.find((s) =>
      [...s.options].some((o) => o.value === "gpt-4o-mini-tts")
    );
    if (!modelSelect) throw new Error("Model select is not visible");
    expect(modelSelect.value).toBe("tts-1");

    // Collapsed options show only their disclosure row; their controls sit
    // behind a hidden wrapper.
    const speedInput = view.container.querySelector('input[type="number"]');
    if (!(speedInput instanceof HTMLInputElement)) throw new Error("Speed input missing");
    expect(speedInput.closest("[hidden]")).not.toBeNull();
    const speed = view.getByRole("button", { name: "Speed" });
    expect(speed.getAttribute("aria-expanded")).toBe("false");
    expect(view.getByRole("button", { name: "Response Format" })).toBeTruthy();
    expect(view.getByRole("button", { name: "Instructions" })).toBeTruthy();

    view.unmount();
  });

  it("expands a collapsed option in place without touching the others", async () => {
    const view = await renderOpenAIProfile();

    await fireEvent.click(view.getByRole("button", { name: "Speed" }));
    const speedInput = view.container.querySelector('input[type="number"]');
    if (!(speedInput instanceof HTMLInputElement)) throw new Error("Speed input did not appear");
    expect(speedInput.value).toBe("1");
    expect(speedInput.closest("[hidden]")).toBeNull();
    expect(view.getByRole("button", { name: "Speed" }).getAttribute("aria-expanded")).toBe("true");

    // The other collapsed options stayed collapsed.
    expect(visibleSelects(view)).toHaveLength(3);

    view.unmount();
  });
});
