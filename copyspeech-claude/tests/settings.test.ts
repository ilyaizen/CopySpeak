import { expect, mock, test } from "claude-code/testing";

// The captions pane, docked beside the transcript.
const PANE = {
  plugin: "copyspeech",
  surface: "desktop",
  component: "Pane",
  requestId: "captions",
  props: {
    title: "CopySpeech companion",
    isFocused: false,
    bodyColumns: 60,
    placement: "dock",
    scroll: { offset: 0, bodyRows: 20 },
    view: {}
  }
} as const;

const OPTIONS = {
  autoRead: false,
  skipSections: true,
  excerpt: true,
  excerptMode: "both",
  excerptTop: 2,
  excerptBottom: 2,
  captions: true
};

// The boolean settings; the rest are choices or numbers.
const BOOLEAN_FIELDS = new Set(["autoRead", "skipSections", "excerpt", "captions"]);

const RUN = {
  command: "copyspeech-settings",
  args: "",
  origin: { kind: "composer" },
  presentation: { isFullscreen: false, columns: 80 }
} as const;

test(
  "the captions pane's settings write the plugin's own rows",
  { options: OPTIONS },
  async ($, on) => {
    const values = new Map<string, boolean | number | string>(Object.entries(OPTIONS));
    const writes: [string, unknown][] = [];
    on("config.list", () => ({
      value: [...values].map(([field, value]) => ({
        key: `copyspeech@inline.${field}`,
        label: field,
        kind: BOOLEAN_FIELDS.has(field) ? "boolean" : "choice",
        value,
        provider: { plugin: "copyspeech@inline", tier: "user" },
        isLocked: false
      }))
    }));
    on("config.set", (_, e) => {
      writes.push([e.key, e.value]);
      // SAFETY: this test writes only its OPTIONS values, which are booleans, numbers, or strings.
      values.set(e.key.slice(e.key.lastIndexOf(".") + 1), e.value as boolean | number | string);
      return { value: e.value };
    });

    const clock = mock.clock(on);
    const pane = await $.ui.mount(PANE);
    await pane.press({ key: "settings" });
    await clock.advance(1);
    await pane.press({ key: "captions" });
    await clock.advance(1);
    await pane.press({ key: "excerptTop-up" });
    await clock.advance(1);
    await pane.press({ key: "excerptMode" });
    await clock.advance(1);
    expect(writes).toEqual([
      ["copyspeech@inline.captions", false],
      ["copyspeech@inline.excerptTop", 3],
      ["copyspeech@inline.excerptMode", "top"]
    ]);
    await pane.unmount();
  }
);

test("/copyspeech-settings opens the captions pane", async ($, on) => {
  const opened: string[] = [];
  on("ui.open", (_, e) => {
    opened.push(e.id);
    return { value: { isPlaced: true } };
  });
  const result = await $.command.run(RUN);
  expect(opened).toEqual(["captions"]);
  expect(result.text).toBe("CopySpeech settings are in the companion pane.");
});
