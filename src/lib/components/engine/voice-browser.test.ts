import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import VoiceBrowser from "./voice-browser.svelte";
import type { VoiceCatalogEntry } from "#lib/types";

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

// The preview button is a sibling of its option (never nested in it) and is
// hidden from assistive tech, so it is found through the option.
function previewFor(label: string): HTMLButtonElement {
  const el = optionFor(label).parentElement?.querySelector<HTMLButtonElement>(
    'button[aria-hidden="true"]'
  );
  if (!el) throw new Error(`preview for ${label} missing`);
  return el;
}

function chip(name: string): HTMLButtonElement {
  const el = [...document.querySelectorAll<HTMLButtonElement>("button[aria-pressed]")].find(
    (b) => b.textContent?.trim() === name
  );
  if (!el) throw new Error(`chip ${name} missing`);
  return el;
}

describe("voice browser filters", () => {
  it("narrows by gender and reports the shown count", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    expect(optionNames()).toEqual(["Ada", "Bea", "Cal", "Dan"]);
    expect(sheet().textContent).toContain("4 of 4");

    await fireEvent.click(chip("Female"));
    expect(optionNames()).toEqual(["Ada", "Bea"]);
    expect(chip("Female").getAttribute("aria-pressed")).toBe("true");
    expect(sheet().textContent).toContain("2 of 4");
  });

  it("narrows by language", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(chip("en"));
    // Dan has no language, so a language filter leaves him out.
    expect(optionNames()).toEqual(["Ada", "Cal"]);
  });

  it("ANDs selected style tags and toggles them off", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(chip("narration"));
    expect(optionNames()).toEqual(["Ada", "Cal"]);
    await fireEvent.click(chip("calm"));
    expect(optionNames()).toEqual(["Ada"]);
    await fireEvent.click(chip("calm"));
    expect(optionNames()).toEqual(["Ada", "Cal"]);
  });

  it("matches search text against tags and reports no match", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.input(search(), { target: { value: "conversational" } });
    expect(optionNames()).toEqual(["Bea"]);

    await fireEvent.input(search(), { target: { value: "zzz" } });
    expect(optionNames()).toEqual([]);
    expect(sheet().textContent).toContain("No voices match “zzz”.");
  });

  it("says so when the filters, not the search, leave nothing", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(chip("Female"));
    await fireEvent.click(chip("young"));
    expect(optionNames()).toEqual([]);
    expect(sheet().textContent).toContain("No voices match these filters.");
  });
});

describe("voice browser previews", () => {
  it("plays one preview at a time and toggles the playing one off", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
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

  it("previews the highlighted voice on Shift+Enter without selecting it", async () => {
    const picked: string[] = [];
    const view = render(VoiceBrowser, {
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

  it("says a voice without a preview has none, and a failed play reports it", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "m2", onselect: () => {} });
    await fireEvent.click(trigger(view.container));
    // Dan is highlighted (saved voice) and has no preview.
    await fireEvent.keyDown(search(), { key: "Enter", shiftKey: true });
    expect(sheet().textContent).toContain("This voice has no preview.");
    expect(playMock).not.toHaveBeenCalled();

    playMock.mockRejectedValueOnce(new Error("network"));
    await fireEvent.click(previewFor("Ada"));
    await waitFor(() => expect(sheet().textContent).toContain("Couldn't play this preview."));
  });

  it("stops the preview when the sheet closes", async () => {
    const view = render(VoiceBrowser, { voices: VOICES, value: "f1", onselect: () => {} });
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
    const view = render(VoiceBrowser, { voices: many(250), value: "v0", onselect: () => {} });
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
    const view = render(VoiceBrowser, { voices: many(250), value: "v0", onselect: () => {} });
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
    const view = render(VoiceBrowser, { voices: many(250), value: "v0", onselect: () => {} });
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

  it("calls onrefresh from the Refresh button and onmanual from the custom id link", async () => {
    const refreshed = vi.fn();
    const manual = vi.fn();
    const view = render(VoiceBrowser, {
      voices: VOICES,
      value: "f1",
      supportsRefresh: true,
      onrefresh: refreshed,
      onmanual: manual,
      onselect: () => {}
    });
    await fireEvent.click(trigger(view.container));
    await fireEvent.click(sheetButton("Refresh"));
    expect(refreshed).toHaveBeenCalledTimes(1);

    await fireEvent.click(sheetButton("Custom / manual id"));
    expect(manual).toHaveBeenCalledTimes(1);
  });
});
