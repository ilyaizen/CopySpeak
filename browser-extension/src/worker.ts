import { parseEvent, parseCaptureReply, ReadingOwner, type CopyCommand } from "./protocol";

// The wire message type derives from tabs.sendMessage's own contract, so the
// deliverables keep chrome's declared shape instead of a hand-rolled dictionary.
type DeliverableMessage = Parameters<typeof chrome.tabs.sendMessage>[1];
type Route = {
  tabId: number;
  frameId: number;
  documentId: string;
  token: string;
  owner: ReadingOwner;
  timer: ReturnType<typeof setTimeout> | null;
};
let route: Route | null = null;
let port: chrome.runtime.Port | null = null;
let automatic = false;
let invocation = 0;
const badge = (text: string, title: string) =>
  Promise.all([chrome.action.setBadgeText({ text }), chrome.action.setTitle({ title })]);
async function deliver(r: Route, message: DeliverableMessage) {
  await chrome.tabs.sendMessage(
    r.tabId,
    { ...message, document_token: r.token },
    { documentId: r.documentId }
  );
}
async function release(disconnected = false) {
  const old = route;
  route = null;
  if (old?.timer) clearTimeout(old.timer);
  if (old) await deliver(old, { type: "disconnect" }).catch(() => {});
  if (!automatic && port) {
    const p = port;
    port = null;
    p.disconnect();
  }
  if (disconnected)
    await badge(
      "!",
      "Open CopySpeak and check native-host installation. Invoke Read selection to retry."
    );
}
function connect() {
  if (port) return port;
  const p = chrome.runtime.connectNative("com.copyspeak.browser");
  port = p;
  p.onDisconnect.addListener(() => {
    if (port !== p) return;
    port = null;
    void release(true);
  });
  p.onMessage.addListener(async (value) => {
    if (port !== p) return;
    const e = parseEvent(value);
    if (!e) {
      p.disconnect();
      port = null;
      await release(true);
      return;
    }
    if (e.type === "probe") {
      if (automatic) await capture(undefined, e.request_id);
      return;
    }
    const r = route;
    if (!r || !r.owner.apply(e)) return;
    if (r.timer) {
      clearTimeout(r.timer);
      r.timer = null;
    }
    await deliver(r, { type: "native", event: e }).catch(() => {
      if (r.owner.readingId)
        p.postMessage({ v: 1, type: "control", reading_id: r.owner.readingId, action: "stop" });
    });
    if (r.owner.ended) await release();
    else await badge("", "CopySpeak · Reading selected text");
  });
  p.postMessage({ v: 1, type: "hello", automatic });
  return p;
}
async function capture(tab?: chrome.tabs.Tab, probe?: string) {
  const generation = ++invocation;
  try {
    if (!tab) tab = (await chrome.tabs.query({ active: true, lastFocusedWindow: true }))[0];
    if (tab?.id === undefined) return;
    if (probe) {
      if (!(await chrome.windows.getLastFocused()).focused) return;
      if (!tab.url || !/^https?:/.test(tab.url)) return;
      if (!(await chrome.permissions.contains({ origins: [new URL(tab.url).origin + "/*"] })))
        return;
    }
    const requestId = probe ?? crypto.randomUUID();
    const frames = await chrome.scripting.executeScript({
      target: { tabId: tab.id, allFrames: true },
      files: ["content.js"]
    });
    const candidates = await Promise.all(
      frames.map(async (frame) => {
        if (!frame.documentId) return null;
        const result = await chrome.tabs
          .sendMessage(
            tab!.id!,
            { type: "capture", request_id: requestId, probe: !!probe },
            { documentId: frame.documentId }
          )
          .catch(() => null);
        const reply = result === null || result === undefined ? null : parseCaptureReply(result);
        if (reply === null) return null;
        return { frame, result: reply };
      })
    );
    const eligible = candidates.filter((x) => x !== null);
    if (generation !== invocation) return;
    if (eligible.length !== 1) {
      await badge("!", "Select text in one permitted, ordinary HTML frame.");
      return;
    }
    if (route?.owner.readingId)
      port?.postMessage({
        v: 1,
        type: "control",
        reading_id: route.owner.readingId,
        action: "stop"
      });
    await release();
    const { frame, result } = eligible[0]!;
    const r: Route = {
      tabId: tab.id,
      frameId: frame.frameId,
      documentId: frame.documentId!,
      token: result.document_token,
      owner: new ReadingOwner(requestId),
      timer: null
    };
    route = r;
    r.timer = setTimeout(
      () => {
        if (route === r) void release(true);
      },
      probe ? 1500 : 15000
    );
    connect().postMessage({
      v: 1,
      type: probe ? "capture" : "start",
      request_id: requestId,
      text: result.text
    });
  } catch {
    await release(true);
  }
}
chrome.action.onClicked.addListener((tab) => capture(tab));
chrome.commands.onCommand.addListener(async (command) => {
  if (command === "read-selection") await capture();
});
chrome.runtime.onMessage.addListener((message: CopyCommand, sender) => {
  const r = route;
  if (message.type === "settings" && !sender.tab?.url?.startsWith("http")) {
    void initialize();
    return;
  }
  if (message.type !== "control") return;
  if (
    !r ||
    sender.tab?.id !== r.tabId ||
    sender.frameId !== r.frameId ||
    sender.documentId !== r.documentId ||
    message.document_token !== r.token ||
    message.reading_id !== r.owner.readingId
  )
    return;
  port?.postMessage({ v: 1, type: "control", reading_id: r.owner.readingId, action: message.action });
});
chrome.tabs.onRemoved.addListener((tabId) => {
  if (route?.tabId === tabId) {
    if (route.owner.readingId)
      port?.postMessage({
        v: 1,
        type: "control",
        reading_id: route.owner.readingId,
        action: "stop"
      });
    void release();
  }
});
chrome.tabs.onUpdated.addListener((tabId, change) => {
  if (route?.tabId === tabId && change.status === "loading") {
    if (route.owner.readingId)
      port?.postMessage({
        v: 1,
        type: "control",
        reading_id: route.owner.readingId,
        action: "stop"
      });
    void release();
  }
});
chrome.permissions.onRemoved.addListener(() => {
  automatic = false;
  port?.postMessage({ v: 1, type: "hello", automatic: false });
  void release();
});
async function initialize() {
  const settings = await chrome.storage.local.get("automatic");
  automatic = settings.automatic === true;
  if (automatic) connect();
  else if (!route) await release();
}
chrome.runtime.onStartup.addListener(() => {
  void initialize();
});
chrome.runtime.onInstalled.addListener(() => {
  void initialize();
});
void initialize();
