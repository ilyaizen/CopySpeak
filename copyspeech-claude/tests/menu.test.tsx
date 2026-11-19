import { expect, mock, test } from "claude-code/testing";

const MENU = {
  plugin: "copyspeech",
  surface: "desktop",
  component: "ContextMenu"
} as const;

test("a selection in a reply gets a row that speaks only the selection", async ($, on) => {
  const clock = mock.clock(on);
  const posts: { url: string; body: string }[] = [];
  on("http.fetch", (_, e) => {
    // The body is the mod's JSON string; stringify keeps the type check honest.
    posts.push({ url: e.url, body: JSON.stringify(e.init?.body ?? "") });
    return { value: { status: 200, ok: true, headers: {}, text: "{}" } };
  });

  const ui = await $.ui.mount({
    ...MENU,
    props: {
      row: { component: "AssistantMessage", requestId: "reply-1" },
      selection: { text: "only these words" }
    }
  });
  await ui.press({ key: "read-selection" });
  await clock.settle();

  const speak = posts.find((p) => p.url.endsWith("/speak"));
  expect(speak?.body).toContain('\\"text\\":\\"only these words\\"');
  expect(speak?.body).toContain('\\"wait\\":false');
  await ui.unmount();
});

test("no selection gets no row", async ($) => {
  const ui = await $.ui.mount({
    ...MENU,
    props: { link: { href: "https://example.com" } }
  });

  expect(await ui.find({ type: "Button" })).toBeUndefined();
  await ui.unmount();
});
