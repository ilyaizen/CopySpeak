import { expect, test } from "claude-code/testing";

const RUN = {
  command: "copyspeech-autoread",
  args: "",
  origin: { kind: "composer" },
  presentation: { isFullscreen: false, columns: 80 }
} as const;

test("/copyspeech-autoread flips the plugin's own config row", async ($, on) => {
  let stored: boolean | string | number | readonly string[] = false;
  const keys: string[] = [];
  on("config.list", () => ({
    value: [
      {
        key: "theme",
        label: "Theme",
        kind: "choice",
        value: "dark",
        provider: { plugin: "engine", tier: "core" },
        isLocked: false
      },
      {
        key: "copyspeech@inline.autoRead",
        label: "Read new replies aloud",
        kind: "boolean",
        value: stored,
        provider: { plugin: "copyspeech@inline", tier: "user" },
        isLocked: false
      }
    ]
  }));
  on("config.set", (_, e) => {
    keys.push(e.key);
    stored = e.value;
    return { value: e.value };
  });

  const first = await $.command.run(RUN);
  expect(first.text).toBe("CopySpeech auto-read is on.");
  expect(stored).toBe(true);
  const second = await $.command.run(RUN);
  expect(second.text).toBe("CopySpeech auto-read is off.");
  expect(stored).toBe(false);
  expect(keys).toEqual(["copyspeech@inline.autoRead", "copyspeech@inline.autoRead"]);
});
