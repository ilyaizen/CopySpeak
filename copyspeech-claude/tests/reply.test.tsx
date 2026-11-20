import { expect, mock, test } from "claude-code/testing";
import type { TestBody } from "claude-code/testing";

const REPLY = { text: "Hello there.", isFirstOfReply: true } as const;

test("desktop reply draws the CopySpeech mark inside the button, in room the label leaves", async ($) => {
  const ui = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: REPLY
  });

  expect(await ui.find({ type: "Svg" })).toBeDefined();
  expect(await ui.find({ key: "play-reply-1", text: /^  Read aloud$/ })).toBeDefined();
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
  expect(await ui.find({ key: "play-reply-1", text: "▶ Read aloud" })).toBeDefined();
  await ui.unmount();
});

test("a playing reply shows Pause and Stop; Pause posts /pause, Stop posts a global /stop", async ($, on) => {
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
  expect(await ui.find({ key: "pause-reply-1" })).toBeUndefined();

  await ui.press({ key: "play-reply-1" });
  await clock.settle();
  await ui.redraw();
  expect(await ui.find({ key: "pause-reply-1" })).toBeDefined();

  await ui.press({ key: "pause-reply-1" });
  await clock.settle();
  expect(urls.some((u) => u.endsWith("/pause"))).toBe(true);

  await ui.press({ key: "stop-reply-1" });
  await clock.settle();
  const stop = urls.lastIndexOf(urls.find((u) => u.endsWith("/stop")) ?? "");
  expect(stop).toBeGreaterThan(-1);
  // No request_id: CopySpeech's global stop, which also aborts queued synthesis.
  expect(bodies[stop]).toBe(JSON.stringify("{}"));
  await ui.redraw();
  expect(await ui.find({ key: "pause-reply-1" })).toBeUndefined();
  await ui.unmount();
});

// /status speaks "Hello there." with "there" under the clock; Read aloud starts the poll.
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
  await playing.press({ key: "play-reply-1" });
  await clock.advance(250);
  await playing.redraw();
  await other.redraw();
  return { playing, other };
}

test("the reply being read draws as Text with a chip on the spoken word; others keep the engine's row", async ($, on) => {
  const { playing, other } = await playHello($, on);
  const word = await playing.find({ type: "Text", text: /^there$/ });
  expect(word?.props).toEqual({ backgroundColor: "claude", color: "inverseText" });
  expect(await playing.find({ key: "stop-reply-1" })).toBeDefined();
  expect(await other.find({ type: "Text", text: "there" })).toBeUndefined();
  expect(await other.find({ key: "stop-reply-2" })).toBeUndefined();
  await other.unmount();
  await playing.unmount();
});
