import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import VoiceBrowser from "../voice-browser.svelte";
import type { VoiceCatalogEntry, VoiceProfile } from "#lib/types";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const profile: VoiceProfile = {
  id: "test-profile",
  name: "Unsaved profile",
  description: null,
  engine: "kokoro",
  voice: "af_heart",
  voice_label: null,
  speed: 1.2,
  pitch: 1,
  effects: { enabled: false, active_effect: "none" },
  post_process: { enabled: false, active_template_id: null },
  engine_options: {}
};
const wav = btoa("RIFF0000WAVE");
const generated = { audio_base64: wav, extension: "wav" };

interface Traits {
  gender?: string;
  language?: string;
  tags?: string[];
  preview?: string;
}

function voice(id: string, label: string, traits: Traits = {}): VoiceCatalogEntry {
  return {
    id,
    label,
    language: traits.language ?? null,
    description: null,
    gender: traits.gender ?? null,
    preview_url: traits.preview ?? null,
    tags: traits.tags ?? []
  };
}

const ADA = voice("f1", "Ada", {
  gender: "female",
  language: "en",
  tags: ["narration", "calm"],
  preview: "https://cdn.test/ada.mp3"
});
const BEA = voice("f2", "Bea", { gender: "female", language: "he", tags: ["conversational"] });
const CAL = voice("m1", "Cal", {
  gender: "male",
  language: "en",
  tags: ["narration", "young"],
  preview: "https://cdn.test/cal.mp3"
});
const DAN = voice("m2", "Dan");
const VOICES = [ADA, BEA, CAL, DAN];

// jsdom has no media playback or layout: stub play/pause and scrolling.
const playMock = vi.fn(() => Promise.resolve());
const pauseMock = vi.fn();

beforeAll(async () => {
  await waitForI18nReady();
});

beforeEach(() => {
  Element.prototype.scrollIntoView = () => {};
  HTMLMediaElement.prototype.play = playMock;
  HTMLMediaElement.prototype.pause = pauseMock;
  playMock.mockClear();
  pauseMock.mockClear();
  invokeMock.mockReset();
});

afterEach(() => {
  cleanup();
});

function trigger(container: HTMLElement): HTMLElement {
  const el = container.querySelector<HTMLElement>('button[aria-haspopup="dialog"]');
  if (!el) throw new Error("trigger missing");
  return el;
}

function sheet(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[role="dialog"]');
  if (!el) throw new Error("sheet is not open");
  return el;
}

function sheetButton(text: string): HTMLButtonElement {
  const el = [...sheet().querySelectorAll<HTMLButtonElement>("button")].find((b) =>
    b.textContent?.includes(text)
  );
  if (!el) throw new Error(`sheet button ${text} missing`);
  return el;
}

function search(): HTMLInputElement {
  const el = document.querySelector<HTMLInputElement>('[role="combobox"]');
  if (!el) throw new Error("search input missing");
  return el;
}

function options(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[role="option"]')];
}

// Labels in these fixtures are one word, so the first token is the name.
function optionNames(): string[] {
  return options().map((el) => el.textContent?.trim().split(/\s+/)[0] ?? "");
}

function optionFor(label: string): HTMLElement {
  const el = options().find((o) => o.textContent?.trim().split(/\s+/)[0] === label);
  if (!el) throw new Error(`option ${label} missing`);
  return el;
}

// Preview is separate from selection and available to keyboard users.
function previewFor(label: string): HTMLButtonElement {
  const el = optionFor(label).parentElement?.querySelector<HTMLButtonElement>("button[aria-label]");
  if (!el) throw new Error(`preview for ${label} missing`);
  return el;
}

// Facet filters are native selects labelled Gender, Language and Style.
function facet(name: string): HTMLSelectElement {
  const el = sheet().querySelector<HTMLSelectElement>(`select[aria-label="${name}"]`);
  if (!el) throw new Error(`facet ${name} missing`);
  return el;
}

describe("voice browser filters", () => {
  it("narrows by gender and reports the shown count", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    expect(optionNames()).toEqual(["Ada", "Bea", "Cal", "Dan"]);
    expect(sheet().textContent).toContain("4 of 4");

    await fireEvent.change(facet("Gender"), { target: { value: "female" } });
    expect(optionNames()).toEqual(["Ada", "Bea"]);
    expect(facet("Gender").value).toBe("female");
    expect(sheet().textContent).toContain("2 of 4");

    await fireEvent.change(facet("Gender"), { target: { value: "" } });
    expect(optionNames()).toEqual(["Ada", "Bea", "Cal", "Dan"]);
  });

  it("narrows by language", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.change(facet("Language"), { target: { value: "en" } });
    // Dan has no language, so a language filter leaves him out.
    expect(optionNames()).toEqual(["Ada", "Cal"]);
  });

  it("narrows by a style tag and returns to all styles", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.change(facet("Style"), { target: { value: "narration" } });
    expect(optionNames()).toEqual(["Ada", "Cal"]);
    await fireEvent.change(facet("Style"), { target: { value: "calm" } });
    expect(optionNames()).toEqual(["Ada"]);
    await fireEvent.change(facet("Style"), { target: { value: "" } });
    expect(optionNames()).toEqual(["Ada", "Bea", "Cal", "Dan"]);
  });

  it("matches search text against tags and reports no match", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.input(search(), { target: { value: "conversational" } });
    expect(optionNames()).toEqual(["Bea"]);

    await fireEvent.input(search(), { target: { value: "zzz" } });
    expect(optionNames()).toEqual([]);
    expect(sheet().textContent).toContain("No voices match “zzz”.");
  });

  it("says so when the filters, not the search, leave nothing", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.change(facet("Gender"), { target: { value: "female" } });
    await fireEvent.change(facet("Style"), { target: { value: "young" } });
    expect(optionNames()).toEqual([]);
    expect(sheet().textContent).toContain("No voices match these filters.");
  });
});

describe("voice browser previews", () => {
  it("warns about provider credits before generating previews", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "m2", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    expect(sheet().textContent).toContain("Generated previews may use provider credits.");
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("plays one preview at a time and toggles the playing one off", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));

    await fireEvent.click(previewFor("Ada"));
    expect(playMock).toHaveBeenCalledTimes(1);
    expect(view.container.querySelector("audio")?.src).toContain("ada.mp3");
    expect(previewFor("Ada").title).toBe("Stop preview");

    await fireEvent.click(previewFor("Cal"));
    expect(playMock).toHaveBeenCalledTimes(2);
    expect(previewFor("Ada").title).toBe("Preview");
    expect(previewFor("Cal").title).toBe("Stop preview");

    await fireEvent.click(previewFor("Cal"));
    expect(pauseMock).toHaveBeenCalled();
    expect(previewFor("Cal").title).toBe("Preview");
  });

  it("ignores a stale failure when the same preview restarts", async () => {
    const first = Promise.withResolvers<void>();
    playMock.mockReturnValueOnce(first.promise);
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Ada"));
    await fireEvent.click(previewFor("Ada"));
    await fireEvent.click(previewFor("Ada"));
    first.reject(new Error("interrupted"));
    await Promise.resolve();
    expect(previewFor("Ada").getAttribute("aria-label")).toBe("Stop preview: Ada");
    expect(sheet().textContent).not.toContain("Couldn't play this preview.");
  });

  it("previews the highlighted voice on Shift+Enter without selecting it", async () => {
    const picked: string[] = [];
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "f1",
      onselect: (id: string) => picked.push(id)
    });
    await fireEvent.click(trigger(view.container));
    await fireEvent.keyDown(search(), { key: "Enter", shiftKey: true });
    expect(playMock).toHaveBeenCalledTimes(1);
    expect(picked).toEqual([]);
    expect(sheet().isConnected).toBe(true);
  });

  it("generates missing previews with the unsaved profile without selecting", async () => {
    invokeMock.mockResolvedValueOnce(generated);
    const picked = vi.fn();
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "m2", onselect: picked });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Dan"));
    await waitFor(() => expect(playMock).toHaveBeenCalled());
    expect(invokeMock).toHaveBeenCalledWith("preview_voice", { profile, voice: "m2" });
    expect(view.container.querySelector("audio")?.src).toBe(`data:audio/wav;base64,${wav}`);
    expect(picked).not.toHaveBeenCalled();
  });

  it("requires a new action to generate after a provider preview fails", async () => {
    playMock.mockRejectedValueOnce(new Error("network"));
    invokeMock.mockResolvedValueOnce(generated);
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Ada"));
    await waitFor(() => expect(sheet().textContent).toContain("Couldn't play this preview."));
    expect(invokeMock).not.toHaveBeenCalled();
    expect(previewFor("Ada").getAttribute("aria-label")).toBe("Generate preview: Ada");
    await fireEvent.click(previewFor("Ada"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("preview_voice", { profile, voice: "f1" })
    );
  });

  it("ignores generated audio after another preview starts or the drawer closes", async () => {
    const first = Promise.withResolvers<typeof generated>();
    const second = Promise.withResolvers<typeof generated>();
    invokeMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Bea"));
    await fireEvent.click(previewFor("Dan"));
    first.resolve(generated);
    await Promise.resolve();
    expect(playMock).not.toHaveBeenCalled();
    await fireEvent.keyDown(sheet(), { key: "Escape" });
    second.resolve(generated);
    await Promise.resolve();
    expect(playMock).not.toHaveBeenCalled();
  });

  it("does not play generated audio after a voice is selected", async () => {
    const pending = Promise.withResolvers<typeof generated>();
    invokeMock.mockReturnValueOnce(pending.promise);
    const picked = vi.fn();
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: picked });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Bea"));
    await fireEvent.click(optionFor("Dan"));
    pending.resolve(generated);
    await Promise.resolve();
    expect(picked).toHaveBeenCalledWith("m2");
    expect(playMock).not.toHaveBeenCalled();
  });

  it("detects MP3 bytes even when the backend extension differs", async () => {
    const mp3 = btoa("ID3preview");
    invokeMock.mockResolvedValueOnce({ audio_base64: mp3, extension: "wav" });
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "m2", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Dan"));
    await waitFor(() => expect(playMock).toHaveBeenCalled());
    expect(view.container.querySelector("audio")?.src).toBe(`data:audio/mpeg;base64,${mp3}`);
  });

  it("shows generated preview failures", async () => {
    invokeMock.mockRejectedValueOnce(new Error("failed"));
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "m2", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Dan"));
    await waitFor(() => expect(sheet().textContent).toContain("Couldn't play this preview."));
  });

  it("stops the preview when the sheet closes", async () => {
    const view = render(VoiceBrowser, { profile, voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(previewFor("Ada"));
    await fireEvent.keyDown(sheet(), { key: "Escape" });
    await waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
    expect(pauseMock).toHaveBeenCalled();

    await fireEvent.click(trigger(view.container));
    expect(previewFor("Ada").title).toBe("Preview");
  });
});

describe("voice browser selection", () => {
  it("selects a clicked row and closes the sheet", async () => {
    const picked: string[] = [];
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "f1",
      onselect: (id: string) => picked.push(id)
    });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(optionFor("Bea"));
    expect(picked).toEqual(["f2"]);
    await waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  });

  it("moves the highlight with arrows, Home and End, and selects it on Enter", async () => {
    const labelled = Array.from({ length: 30 }, (_, i) => voice(`v${i}`, `Voice${i}`));
    const picked: string[] = [];
    const view = render(VoiceBrowser, {
      profile,
      voices: labelled,
      value: "v1",
      onselect: (id: string) => picked.push(id)
    });
    await fireEvent.click(trigger(view.container));
    const active = () => {
      const id = search().getAttribute("aria-activedescendant");
      return id ? document.getElementById(id)?.textContent?.trim() : null;
    };

    // Opens on the saved voice.
    expect(active()).toBe("Voice1");
    await fireEvent.keyDown(search(), { key: "ArrowDown" });
    expect(active()).toBe("Voice2");
    await fireEvent.keyDown(search(), { key: "ArrowUp" });
    await fireEvent.keyDown(search(), { key: "ArrowUp" });
    await fireEvent.keyDown(search(), { key: "ArrowUp" });
    expect(active()).toBe("Voice0");
    await fireEvent.keyDown(search(), { key: "End" });
    expect(active()).toBe("Voice29");
    await fireEvent.keyDown(search(), { key: "Home" });
    expect(active()).toBe("Voice0");

    // Filtering restarts at the top match.
    await fireEvent.input(search(), { target: { value: "Voice2" } });
    expect(active()).toBe("Voice2");
    await fireEvent.keyDown(search(), { key: "ArrowDown" });
    expect(active()).toBe("Voice20");
    await fireEvent.keyDown(search(), { key: "Enter" });
    expect(picked).toEqual(["v20"]);
    await waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  });

  it("shows a saved voice the list lacks, with a Not in list note", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "gone",
      valueLabel: "Old Name",
      onselect: () => {}
    });
    expect(trigger(view.container).textContent).toContain("Old Name");
    await fireEvent.click(trigger(view.container));
    expect(sheet().textContent).toContain("Not in list");
  });
});

describe("voice browser pagination", () => {
  const many = (n: number) => Array.from({ length: n }, (_, i) => voice(`v${i}`, `Voice${i}`));

  it("renders one page and reveals more on demand", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: many(250),
      value: "v0",
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    expect(options()).toHaveLength(100);
    expect(sheet().textContent).toContain("Show 150 more");

    await fireEvent.click(sheetButton("Show 150 more"));
    expect(options()).toHaveLength(200);
    expect(sheet().textContent).toContain("Show 50 more");

    await fireEvent.click(sheetButton("Show 50 more"));
    expect(options()).toHaveLength(250);
    const moreButtons = [...sheet().querySelectorAll("button")].filter((b) =>
      b.textContent?.includes("more")
    );
    expect(moreButtons).toHaveLength(0);
  });

  it("reveals the next page when arrowing past the rendered rows, and all on End", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: many(250),
      value: "v0",
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    const active = () => {
      const id = search().getAttribute("aria-activedescendant");
      return id ? document.getElementById(id)?.textContent?.trim() : null;
    };

    // Walk to the last rendered row, then one more: the next page appears.
    for (let i = 0; i < 100; i += 1) {
      await fireEvent.keyDown(search(), { key: "ArrowDown" });
    }
    expect(active()).toBe("Voice100");
    expect(options()).toHaveLength(200);

    // End jumps to the last match and reveals everything.
    await fireEvent.keyDown(search(), { key: "End" });
    expect(active()).toBe("Voice249");
    expect(options()).toHaveLength(250);
  });

  it("restarts at one page when the search changes", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: many(250),
      value: "v0",
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(sheetButton("Show 150 more"));
    expect(options()).toHaveLength(200);

    // "Voice1" matches Voice1, Voice10-19 and Voice100-199: 111 voices.
    await fireEvent.input(search(), { target: { value: "Voice1" } });
    expect(options()).toHaveLength(100);
    expect(sheet().textContent).toContain("Show 11 more");
  });
});

describe("voice browser refresh and error line", () => {
  it("renders a failed refresh in the sheet's error line", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "f1",
      supportsRefresh: true,
      error: "Failed to fetch voices: HTTP error: 401",
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    expect(sheet().textContent).toContain(
      "Couldn't refresh: Failed to fetch voices: HTTP error: 401"
    );
  });

  it("hides the error line while a refresh is running", async () => {
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "f1",
      supportsRefresh: true,
      loading: true,
      error: "Failed to fetch voices: HTTP error: 401",
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    expect(sheet().textContent).not.toContain("HTTP error: 401");
  });

  it("calls onrefresh from the header Refresh button", async () => {
    const refreshed = vi.fn();
    const view = render(VoiceBrowser, {
      profile,
      voices: VOICES,
      value: "f1",
      supportsRefresh: true,
      onrefresh: refreshed,
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    const refresh = sheet().querySelector<HTMLButtonElement>('button[aria-label="Refresh"]');
    if (!refresh) throw new Error("refresh button missing");
    await fireEvent.click(refresh);
    expect(refreshed).toHaveBeenCalledTimes(1);
  });
});
