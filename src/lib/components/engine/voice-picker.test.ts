import { afterEach, beforeEach, describe, expect, it } from "vite-plus/test";
import { fireEvent, render } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import VoicePicker from "./voice-picker.svelte";
import type { VoiceCatalogEntry } from "#lib/types";

const VOICES: VoiceCatalogEntry[] = Array.from({ length: 30 }, (_, i) => ({
  id: `v${i}`,
  label: `Voice ${i}`,
  language: null,
  description: null,
  gender: null,
  preview_url: null
}));

// jsdom has no layout: stub the trigger's rect and the panel's natural height
// (search + 18rem list + refresh footer). An inline max-height caps it.
const PANEL_NATURAL_HEIGHT = 370;
const originalRect = HTMLElement.prototype.getBoundingClientRect;
const originalOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
let trigger: HTMLElement | null = null;
let triggerTop = 0;

function isPanel(el: HTMLElement): boolean {
  return el.parentElement === document.body && el.querySelector('[role="listbox"]') !== null;
}

function panelHeight(el: HTMLElement): number {
  const max = Number.parseFloat(el.style.maxHeight);
  return Number.isFinite(max) ? Math.min(max, PANEL_NATURAL_HEIGHT) : PANEL_NATURAL_HEIGHT;
}

beforeEach(async () => {
  await waitForI18nReady();
  // jsdom has no scrolling; keyboard navigation scrolls the active row into view.
  Element.prototype.scrollIntoView = () => {};
  HTMLElement.prototype.getBoundingClientRect = function () {
    if (this === trigger) return new DOMRect(100, triggerTop, 224, 36);
    if (isPanel(this)) {
      const top = Number.parseFloat(this.style.top) || 0;
      return new DOMRect(100, top, 288, panelHeight(this));
    }
    return originalRect.call(this);
  };
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get() {
      return isPanel(this) ? panelHeight(this) : 0;
    }
  });
});

afterEach(() => {
  trigger = null;
  HTMLElement.prototype.getBoundingClientRect = originalRect;
  if (originalOffsetHeight) {
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", originalOffsetHeight);
  }
});

async function openAt(top: number, viewportHeight: number) {
  triggerTop = top;
  window.innerHeight = viewportHeight;
  const view = render(VoicePicker, { voices: VOICES, value: "v1", onselect: () => {} });
  trigger = view.container.querySelector("button");
  if (!trigger) throw new Error("trigger missing");
  await fireEvent.click(trigger);
  const panel = [...document.body.children].find(
    (el): el is HTMLElement => el instanceof HTMLElement && isPanel(el)
  );
  if (!panel) throw new Error("voice panel did not open");
  return { view, panel };
}

describe("voice picker placement", () => {
  it("keeps the panel inside a short window when neither side fits it whole", async () => {
    const { view, panel } = await openAt(300, 500);
    const top = Number.parseFloat(panel.style.top);
    expect(top).toBeGreaterThanOrEqual(8);
    expect(top + panelHeight(panel)).toBeLessThanOrEqual(500 - 8);
    view.unmount();
  });

  it("opens upward when there is more room above the trigger", async () => {
    const { view, panel } = await openAt(420, 500);
    const top = Number.parseFloat(panel.style.top);
    expect(top).toBeGreaterThanOrEqual(8);
    expect(top + panelHeight(panel)).toBeLessThanOrEqual(420);
    view.unmount();
  });
});

describe("voice picker keyboard navigation", () => {
  it("moves the active row with arrows, Home and End, and selects it on Enter", async () => {
    triggerTop = 100;
    window.innerHeight = 800;
    const picked: string[] = [];
    const view = render(VoicePicker, {
      voices: VOICES,
      value: "v1",
      onselect: (id: string) => picked.push(id)
    });
    trigger = view.container.querySelector("button");
    if (!trigger) throw new Error("trigger missing");
    await fireEvent.click(trigger);
    const search = document.body.querySelector<HTMLInputElement>('[role="combobox"]');
    if (!search) throw new Error("search input missing");
    const active = () => {
      const id = search.getAttribute("aria-activedescendant");
      return id ? document.getElementById(id)?.textContent?.trim() : null;
    };

    // Opens on the saved voice.
    expect(active()).toBe("Voice 1");
    await fireEvent.keyDown(search, { key: "ArrowDown" });
    expect(active()).toBe("Voice 2");
    await fireEvent.keyDown(search, { key: "ArrowUp" });
    await fireEvent.keyDown(search, { key: "ArrowUp" });
    await fireEvent.keyDown(search, { key: "ArrowUp" });
    expect(active()).toBe("Voice 0");
    await fireEvent.keyDown(search, { key: "End" });
    expect(active()).toBe("Voice 29");
    await fireEvent.keyDown(search, { key: "Home" });
    expect(active()).toBe("Voice 0");

    // Filtering restarts at the top match.
    await fireEvent.input(search, { target: { value: "Voice 2" } });
    expect(active()).toBe("Voice 2");
    await fireEvent.keyDown(search, { key: "ArrowDown" });
    expect(active()).toBe("Voice 20");
    await fireEvent.keyDown(search, { key: "Enter" });
    expect(picked).toEqual(["v20"]);
    expect(document.body.querySelector('[role="combobox"]')).toBeNull();
    view.unmount();
  });
});
