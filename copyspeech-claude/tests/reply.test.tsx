import { expect, mock, test } from "claude-code/testing";
import type { TestBody } from "claude-code/testing";

const REPLY = { text: "Hello there.", isFirstOfReply: true } as const;
// The band above the prompt: one row per plugin, drawn while a reading plays.
const BAND = {
  plugin: "copyspeech",
  surface: "desktop",
  component: "AbovePrompt",
  props: {
    hasSurvey: false,
    isCollapsed: false,
    isWorking: false,
    maxRows: 3,
    bodyColumns: 80,
    scroll: { offset: 0, bodyRows: 3 },
    view: {}
  }
} as const;

test("desktop reply draws the CopySpeech mark inside the button, in room the label leaves", async ($) => {
  const ui = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: REPLY
  });

  expect(await ui.find({ type: "Svg" })).toBeDefined();
  expect(await ui.find({ key: "play-reply-1", text: /^  Read excerpt$/ })).toBeDefined();
  await ui.unmount();
});

test("terminal reply has no Svg and labels the button with a ▶", async ($) => {
  const ui = await $.ui.mount({
    plugin: "copyspeech",
    surface: "terminal",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: REPLY
  });

  expect(await ui.find({ type: "Svg" })).toBeUndefined();
  expect(await ui.find({ key: "play-reply-1", text: "▶ Read excerpt" })).toBeDefined();
  await ui.unmount();
});

test("a playing reply shows Pause and Stop in the band; Pause posts /pause, Stop posts a global /stop", async ($, on) => {
  const clock = mock.clock(on);
  const urls: string[] = [];
  const bodies: string[] = [];
  on("http.fetch", (_, e) => {
    urls.push(e.url);
    bodies.push(JSON.stringify(e.init?.body ?? ""));
    return { value: { status: 200, ok: true, headers: {}, text: "{}" } };
  });
  const ui = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: REPLY
  });
  const band = await $.ui.mount(BAND);
  expect(await band.find({ key: "pause" })).toBeUndefined();

  await ui.press({ key: "play-reply-1" });
  await clock.settle();
  await band.redraw();
  expect(await band.find({ key: "pause" })).toBeDefined();
  expect(await band.find({ key: "stop" })).toBeDefined();

  await band.press({ key: "pause" });
  await clock.settle();
  expect(urls.some((u) => u.endsWith("/pause"))).toBe(true);

  await band.press({ key: "stop" });
  await clock.settle();
  const stop = urls.lastIndexOf(urls.find((u) => u.endsWith("/stop")) ?? "");
  expect(stop).toBeGreaterThan(-1);
  // No request_id: CopySpeech's global stop, which also aborts queued synthesis.
  expect(bodies[stop]).toBe(JSON.stringify("{}"));
  await band.redraw();
  expect(await band.find({ key: "pause" })).toBeUndefined();
  await band.unmount();
  await ui.unmount();
});

test(
  "with both trims off, the play button reads the whole reply and says Read aloud",
  { options: { skipSections: false, excerpt: false } },
  async ($) => {
    const ui = await $.ui.mount({
      plugin: "copyspeech",
      surface: "terminal",
      component: "AssistantMessage",
      requestId: "reply-1",
      props: REPLY
    });
    expect(await ui.find({ key: "play-reply-1", text: "▶ Read aloud" })).toBeDefined();
    await ui.unmount();
  }
);

// /status speaks "Hello there." with "there" under the clock; the play button starts the poll.
// `status` is the value /status returns: a test moves the clock by changing its position_ms.
async function playHello($: Parameters<TestBody>[0], on: Parameters<TestBody>[1]) {
  const clock = mock.clock(on);
  const status = {
    active: true,
    paused: false,
    position_ms: 350,
    fragment_index: 0,
    captions: {
      text: "Hello there.",
      words: [
        { text_start: 0, text_end: 5, start_ms: 0, end_ms: 300 },
        { text_start: 6, text_end: 11, start_ms: 300, end_ms: 600 }
      ]
    }
  };
  on("http.fetch", (_, e) => ({
    value: {
      status: 200,
      ok: true,
      headers: {},
      text: e.url.endsWith("/status") ? JSON.stringify(status) : "{}"
    }
  }));
  on("command.register", (_, e) => ({ value: { command: e.name } }));
  on("session.start", (_, e) => ({ cwd: e.cwd }));
  // Starts the /status poll.
  await $.session.start({ cwd: ".", surface: "desktop", isInteractive: true });
  const mount = (requestId: string) =>
    $.ui.mount({
      plugin: "copyspeech",
      surface: "desktop",
      component: "AssistantMessage",
      requestId,
      props: REPLY
    });
  const playing = await mount("reply-1");
  const other = await mount("reply-2");
  const band = await $.ui.mount(BAND);
  await playing.press({ key: "play-reply-1" });
  await clock.advance(250);
  await playing.redraw();
  await other.redraw();
  await band.redraw();
  return { playing, other, band, status, clock };
}

test("every reply draws as Text; the reading puts a chip on the spoken word and no buttons in the reply", async ($, on) => {
  const { playing, other, band } = await playHello($, on);
  const word = await playing.find({ type: "Text", text: /^there$/ });
  expect(word?.props).toEqual({ backgroundColor: "claude", color: "inverseText" });
  expect(await playing.find({ type: "Markdown" })).toBeUndefined();
  expect(await other.find({ type: "Markdown" })).toBeUndefined();
  expect(await other.find({ type: "Text", text: "Hello there." })).toBeDefined();
  expect(await other.find({ type: "Text", text: /^there$/ })).toBeUndefined();
  expect(await playing.find({ key: "pause-reply-1" })).toBeUndefined();
  expect(await playing.find({ key: "stop-reply-1" })).toBeUndefined();
  expect(await other.find({ key: "stop-reply-2" })).toBeUndefined();
  expect(await band.find({ key: "stop" })).toBeDefined();
  await other.unmount();
  await band.unmount();
  await playing.unmount();
});

test("Pause stays in the band while the spoken word moves in the reply", async ($, on) => {
  const { playing, other, band, status, clock } = await playHello($, on);
  status.position_ms = 100;
  await clock.advance(250);
  await playing.redraw();
  const word = await playing.find({ type: "Text", text: /^Hello$/ });
  expect(word?.props).toEqual({ backgroundColor: "claude", color: "inverseText" });
  await band.redraw();
  expect(await band.find({ key: "pause" })).toBeDefined();
  await other.unmount();
  await band.unmount();
  await playing.unmount();
});
