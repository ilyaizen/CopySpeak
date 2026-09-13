import {
  parseEvent,
  parseCaptureReply,
  ReadingOwner,
  type CopyCommand,
  type NativeEvent
} from "./protocol";

// Deliverables are worker-built objects given to chrome.tabs.sendMessage, which
// types its payload through an unresolved generic; hand-roll the two variants it
// actually sends and let deliver() stamp the document token.
type DeliverableMessage = { type: "disconnect" } | { type: "native"; event: NativeEvent };
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
    if (route !== r) return;
    if (r.owner.ended) {
      await release();
      if (e.type === "rejected") await badge("!", `CopySpeak refused the selection: ${e.reason}`);
      else if (e.type === "state" && e.status === "error")
        await badge("!", "CopySpeak could not read the selection. Check the app for the error.");
    } else await badge("", "CopySpeak · Reading selected text");
  });
  p.postMessage({ v: 1, type: "hello", automatic });
  return p;
}
async function capture(tab?: chrome.tabs.Tab, probe?: string, paragraphDocument?: string) {
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
      target: paragraphDocument
        ? { tabId: tab.id, documentIds: [paragraphDocument] }
        : { tabId: tab.id, allFrames: true },
      files: ["content.js"]
    });
    if (generation !== invocation) return;
    if (route?.owner.readingId)
      port?.postMessage({
        v: 1,
        type: "control",
        reading_id: route.owner.readingId,
        action: "stop"
      });
    // Disconnect before capture installs a replacement anchor in the same document.
    await release();
    if (generation !== invocation) return;
    const candidates = await Promise.all(
      frames.map(async (frame) => {
        if (!frame.documentId) return null;
        const result = await chrome.tabs
          .sendMessage(
            tab!.id!,
            {
              type: "capture",
              request_id: requestId,
              probe: !!probe,
              paragraph: !!paragraphDocument
            },
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
chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "read-selection") await capture(tab);
  if (info.menuItemId === "companion-settings") await chrome.runtime.openOptionsPage();
});
chrome.runtime.onMessage.addListener((message: CopyCommand, sender, respond) => {
  const r = route;
  if (message.type === "site-access") {
    if (!sender.url || !/^https?:/.test(sender.url)) {
      respond(false);
      return;
    }
    void chrome.permissions
      .contains({ origins: [new URL(sender.url).origin + "/*"] })
      .then(respond, () => respond(false));
    return true;
  }
  if (message.type === "settings" && sender.url === chrome.runtime.getURL("options.html")) {
    void initialize().then(
      () => respond({ ok: true }),
      () => respond({ error: "Could not apply settings. Try again." })
    );
    return true;
  }
  if (message.type === "read-paragraph") {
    if (
      sender.tab?.id === undefined ||
      !sender.documentId ||
      !sender.url ||
      !/^https?:/.test(sender.url)
    )
      return;
    const tab = sender.tab;
    const documentId = sender.documentId;
    void chrome.storage.local.get("hoverRead").then(async (settings) => {
      if (
        settings.hoverRead !== true ||
        !(await chrome.permissions.contains({ origins: [new URL(sender.url!).origin + "/*"] }))
      )
        return;
      await capture(tab, undefined, documentId);
    });
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
  port?.postMessage({
    v: 1,
    type: "control",
    reading_id: r.owner.readingId,
    action: message.action
  });
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
  void initialize();
});
chrome.permissions.onAdded.addListener(() => {
  void initialize();
});
// Serialize registration updates when permission and settings events arrive together.
let initialization = Promise.resolve();
function initialize() {
  initialization = initialization.catch(() => {}).then(applySettings);
  return initialization;
}
async function applySettings() {
  const settings = await chrome.storage.local.get(["automatic", "hoverRead"]);
  const origins =
    (await chrome.permissions.getAll()).origins?.filter((origin) => /^https?:\/\//.test(origin)) ??
    [];
  automatic = settings.automatic === true && origins.length > 0;
  port?.postMessage({ v: 1, type: "hello", automatic });
  if (automatic) connect();
  else if (!route) await release();
  const scripts = await chrome.scripting.getRegisteredContentScripts({ ids: ["paragraph-hover"] });
  if (settings.hoverRead === true && origins.length) {
    const script = {
      id: "paragraph-hover",
      matches: origins,
      js: ["content.js"],
      allFrames: true,
      runAt: "document_idle" as const
    };
    if (scripts.length) await chrome.scripting.updateContentScripts([script]);
    else await chrome.scripting.registerContentScripts([script]);
    // Apply immediately to already-open permitted pages; future pages use registration.
    const tabs = await chrome.tabs.query({ url: origins });
    await Promise.all(
      tabs.map(async (tab) => {
        if (tab.id === undefined) return;
        await chrome.scripting
          .executeScript({
            target: { tabId: tab.id, allFrames: true },
            files: ["content.js"]
          })
          .catch(() => {});
      })
    );
  } else if (scripts.length) {
    await chrome.scripting.unregisterContentScripts({ ids: ["paragraph-hover"] });
  }
  const tabs = await chrome.tabs.query({});
  await Promise.all(
    tabs.map(async (tab) => {
      if (tab.id === undefined) return;
      // Ask each injected frame about its own origin; cross-origin frames can have
      // different grants from their containing tab.
      await chrome.tabs.sendMessage(tab.id, { type: "refresh-site-access" }).catch(() => {});
    })
  );
}
chrome.runtime.onStartup.addListener(() => {
  void initialize();
});
chrome.runtime.onInstalled.addListener(() => {
  // Menus persist across worker restarts; reading lastError silences the
  // duplicate-id error an extension reload can raise.
  chrome.contextMenus.create(
    { id: "read-selection", title: "Read with CopySpeak", contexts: ["selection"] },
    () => void chrome.runtime.lastError
  );
  chrome.contextMenus.create(
    { id: "companion-settings", title: "CopySpeak Companion settings", contexts: ["action"] },
    () => void chrome.runtime.lastError
  );
  void initialize();
});
void initialize();
