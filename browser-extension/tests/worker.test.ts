import { test, expect } from "bun:test";
test("worker routes one accepted reading to captured document and ignores forged controls", async () => {
  const sent: any[] = [];
  const delivered: any[] = [];
  type TestHook = (...args: any[]) => void;
  const hooks: Record<string, TestHook> = {};
  const event = (name: string) => ({ addListener: (f: any) => (hooks[name] = f) });
  // SAFETY: the test stubs the chrome global that the worker reads; global typing cannot know it.
  (globalThis as any).chrome = {
    runtime: {
      connectNative: () => ({
        postMessage: (x: any) => sent.push(x),
        disconnect: () => {},
        onMessage: event("native"),
        onDisconnect: event("disconnect")
      }),
      onMessage: event("message"),
      onStartup: event("startup"),
      onInstalled: event("installed"),
      lastError: undefined
    },
    commands: { onCommand: event("command") },
    contextMenus: { create: () => {}, onClicked: event("menu") },
    action: { onClicked: event("click"), setBadgeText: async () => {}, setTitle: async () => {} },
    tabs: {
      query: async () => [{ id: 7, windowId: 1 }],
      sendMessage: async (id: number, m: any, opts: any) => {
        delivered.push({ id, m, opts });
        if (m.type === "capture") return { text: "selected", document_token: "doc" };
      },
      onRemoved: event("removed"),
      onUpdated: event("updated")
    },
    scripting: { executeScript: async () => [{ frameId: 0, documentId: "browser-doc" }] },
    permissions: { contains: async () => false, onRemoved: event("permissions") },
    storage: { local: { get: async () => ({ automatic: false }) } },
    windows: { getLastFocused: async () => ({ focused: true }) }
  };
  await import("../src/worker");
  await Promise.resolve(hooks.click({ id: 7 }));
  expect(sent.filter((x) => x.type === "start").length).toBe(1);
  const request = sent.find((x) => x.type === "start");
  await Promise.resolve(
    hooks.native({
      v: 1,
      type: "accepted",
      request_id: request.request_id,
      reading_id: "reading"
    })
  );
  expect(delivered.at(-1).opts.documentId).toBe("browser-doc");
  expect(delivered.at(-1).m.document_token).toBe("doc");
  hooks.message(
    { type: "control", reading_id: "reading", document_token: "doc", action: "stop" },
    { tab: { id: 8 }, frameId: 0, documentId: "browser-doc" },
    () => {}
  );
  expect(sent.filter((x) => x.type === "control").length).toBe(0);
  hooks.message(
    { type: "control", reading_id: "reading", document_token: "doc", action: "pause" },
    { tab: { id: 7 }, frameId: 0, documentId: "browser-doc" },
    () => {}
  );
  expect(sent.at(-1).action).toBe("pause");
  await Promise.resolve(hooks.disconnect());
  expect(delivered.at(-1).m.type).toBe("disconnect");
  await Promise.resolve(hooks.menu({ menuItemId: "read-selection" }, { id: 7 }));
  expect(sent.filter((x) => x.type === "start").length).toBe(2);
});
