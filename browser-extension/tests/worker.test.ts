import { test, expect } from "bun:test";

// One worker import per bun-test process: the module registry is shared
// across a run, so the top-level initialize and every listener below run
// exactly once, against this one mutable mock. Tests build on each other's
// end state in file order.
const sent: any[] = [];
const delivered: any[] = [];
const badgeTexts: string[] = [];
let connectCount = 0;
type TestHook = (...args: any[]) => void;
const hooks: Record<string, TestHook> = {};
const event = (name: string) => ({ addListener: (f: any) => (hooks[name] = f) });
const settings = { automatic: false };
// SAFETY: the test stubs the chrome global that the worker reads; global typing cannot know it.
(globalThis as any).chrome = {
  runtime: {
    getURL: (path: string) => `chrome-extension://test/${path}`,
    connectNative: () => {
      connectCount++;
      return {
        postMessage: (x: any) => sent.push(x),
        disconnect: () => {},
        onMessage: event("native"),
        onDisconnect: event("disconnect")
      };
    },
    onMessage: event("message"),
    onStartup: event("startup"),
    onInstalled: event("installed"),
    lastError: undefined
  },
  commands: { onCommand: event("command") },
  contextMenus: { create: () => {}, onClicked: event("menu") },
  action: {
    onClicked: event("click"),
    setBadgeText: async ({ text }: any) => {
      badgeTexts.push(text);
    },
    setTitle: async () => {}
  },
  tabs: {
    query: async (query: any) => (query.active ? [{ id: 7, windowId: 1 }] : []),
    sendMessage: async (id: number, m: any, opts: any) => {
      delivered.push({ id, m, opts });
      if (m.type === "capture") return { text: "selected", document_token: "doc" };
    },
    onRemoved: event("removed"),
    onUpdated: event("updated")
  },
  scripting: {
    executeScript: async () => [{ frameId: 0, documentId: "browser-doc" }],
    getRegisteredContentScripts: async () => [],
    registerContentScripts: async () => {},
    updateContentScripts: async () => {},
    unregisterContentScripts: async () => {}
  },
  permissions: {
    contains: async () => false,
    getAll: async () => ({ origins: ["https://example.com/*"] }),
    onRemoved: event("permissions"),
    onAdded: event("permissionsAdded")
  },
  storage: { local: { get: async () => ({ automatic: settings.automatic, hoverRead: false }) } },
  windows: { getLastFocused: async () => ({ focused: true }) }
};
// Macrotask tick: drains every pending microtask before an assertion.
const settle = () => new Promise((r) => setTimeout(r, 0));
await import("../src/worker");
await settle();

test("worker routes one accepted reading to captured document and ignores forged controls", async () => {
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
  await Promise.resolve(hooks.click({ id: 7 }));
  const replacement = delivered.slice(-2).map((entry) => entry.m.type);
  expect(replacement).toEqual(["disconnect", "capture"]);
  expect(sent.filter((entry) => entry.type === "start").length).toBe(2);
  await Promise.resolve(hooks.disconnect());
  expect(delivered.at(-1).m.type).toBe("disconnect");
  await Promise.resolve(hooks.menu({ menuItemId: "read-selection" }, { id: 7 }));
  expect(sent.filter((x) => x.type === "start").length).toBe(3);
});

test("a page-copy wake revives the native-host port a disconnect killed", async () => {
  // Test 1 ended mid-reading with a live port and automatic=false. The ping
  // arrives on a worker whose applySettings never ran with the setting on:
  // initialize re-reads storage, flips automatic on, and posts a fresh hello
  // on the port it already holds.
  settings.automatic = true;
  hooks.message({ type: "wake" }, { tab: { id: 7 }, frameId: 0, documentId: "d" }, () => {});
  await settle();
  expect(connectCount).toBe(3);
  expect(sent.filter((x) => x.type === "hello")).toHaveLength(4);
  // The app quits: the port dies, the worker badges "!" and by design does
  // not reconnect on its own — the state a double-copy used to find.
  await Promise.resolve(hooks.disconnect());
  await settle();
  expect(badgeTexts.at(-1)).toBe("!");
  expect(connectCount).toBe(3);
  // The next copy on a permitted page pings the worker: it must initialize
  // and reconnect, so the trigger that copy starts has a port to land on.
  hooks.message({ type: "wake" }, { tab: { id: 7 }, frameId: 0, documentId: "d" }, () => {});
  await settle();
  expect(connectCount).toBe(4);
  expect(sent.filter((x) => x.type === "hello")).toHaveLength(5);
  // A live connection heals the stale badge.
  expect(badgeTexts.at(-1)).toBe("");
  // A wake with the port alive is free: no duplicate connection.
  hooks.message({ type: "wake" }, { tab: { id: 7 }, frameId: 0, documentId: "d" }, () => {});
  await settle();
  expect(connectCount).toBe(4);
});
