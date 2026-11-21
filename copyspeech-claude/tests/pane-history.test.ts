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

test("the settings sit at the bottom of the pane, collapsed until expanded", async ($, on) => {
  const clock = mock.clock(on);
  const pane = await $.ui.mount(PANE);
  expect(await pane.find({ key: "settings" })).toBeDefined();
  expect(await pane.find({ key: "captions" })).toBeUndefined();
  await pane.press({ key: "settings" });
  await clock.advance(1);
  await pane.redraw();
  expect(await pane.find({ key: "captions" })).toBeDefined();
  await pane.unmount();
});

test("a new Read aloud replaces the reply the pane shows", async ($, on) => {
  const clock = mock.clock(on);
  on("http.fetch", () => ({ value: { status: 200, ok: true, headers: {}, text: "{}" } }));
  const one = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: { text: "Old message here.", isFirstOfReply: true }
  });
  const two = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-2",
    props: { text: "New message here.", isFirstOfReply: true }
  });
  await one.press({ key: "play-reply-1" });
  await clock.advance(250);
  await two.press({ key: "play-reply-2" });
  await clock.advance(250);
  const pane = await $.ui.mount(PANE);
  await pane.redraw();
  expect(await pane.find({ type: "Text", text: /Old/ })).toBeUndefined();
  expect(await pane.find({ type: "Text", text: /New/ })).toBeDefined();
  await pane.unmount();
  await one.unmount();
  await two.unmount();
});

test("a bare inactive status waits out the idle wait before the reading ends", async ($, on) => {
  const clock = mock.clock(on);
  on("command.register", (_, e) => ({ value: { command: e.name } }));
  on("session.start", (_, e) => ({ cwd: e.cwd }));
  let status: Record<string, unknown> = {
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
  const reply = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: { text: "Hello there.", isFirstOfReply: true }
  });
  // Starts the /status poll, which ends the reading.
  await $.session.start({ cwd: ".", surface: "desktop", isInteractive: true });
  await reply.press({ key: "play-reply-1" });
  await clock.advance(250);
  const pane = await $.ui.mount(PANE);
  await pane.redraw();
  expect(await pane.find({ key: "settings" })).toBeUndefined();
  status = { active: false };
  await clock.advance(250);
  await pane.redraw();
  // A bare report is also the app's answer after a silent stretch, so the idle wait still applies.
  expect(await pane.find({ key: "settings" })).toBeUndefined();
  await clock.advance(2500);
  await pane.redraw();
  expect(await pane.find({ key: "settings" })).toBeDefined();
  await pane.unmount();
  await reply.unmount();
});

test("the captions pane keeps the last reading after it stops", async ($, on) => {
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
  const reply = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-1",
    props: { text: "Hello there.", isFirstOfReply: true }
  });
  await reply.press({ key: "play-reply-1" });
  await clock.advance(250);
  const pane = await $.ui.mount(PANE);
  status.active = false;
  await clock.advance(2500);
  await pane.redraw();
  expect(await pane.find({ type: "Text", text: /Hello/ })).toBeDefined();
  expect(await pane.find({ type: "Text", text: "Nothing read yet." })).toBeUndefined();
  await pane.unmount();
  await reply.unmount();
});

test("a paragraph's Read button speaks only that paragraph", async ($, on) => {
  const clock = mock.clock(on);
  const bodies: string[] = [];
  on("http.fetch", (_, e) => {
    if (e.url.endsWith("/speak")) bodies.push(JSON.stringify(e.init?.body ?? ""));
    return { value: { status: 200, ok: true, headers: {}, text: "{}" } };
  });
  const reply = await $.ui.mount({
    plugin: "copyspeech",
    surface: "desktop",
    component: "AssistantMessage",
    requestId: "reply-2",
    props: {
      text: "First paragraph here.\n\nSecond paragraph here.\n\nThird paragraph here.",
      isFirstOfReply: true
    }
  });
  await reply.press({ key: "play-reply-2" });
  await clock.advance(250);
  const pane = await $.ui.mount(PANE);
  await pane.press({ key: "read-reply-2-1" });
  await clock.advance(250);
  const last = bodies[bodies.length - 1] ?? "";
  expect(last).toContain("Second paragraph here");
  expect(last).not.toContain("First paragraph");
  expect(last).not.toContain("Third paragraph");
  await pane.unmount();
  await reply.unmount();
});
