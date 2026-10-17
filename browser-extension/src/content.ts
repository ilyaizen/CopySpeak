import { captureSelection, matchCopiedText, type SelectionAnchor } from "./anchor";
import {
  ReadingOwner,
  TriggerOwner,
  parseEvent,
  type ControlAction,
  type CopyCommand,
  type Status
} from "./protocol";
import { paragraphHover } from "./hover";

// Repeated scripting.executeScript calls do not install duplicate listeners.
const marker = "__copyspeak_companion_v1__";
// SAFETY: only adds the optional script-unique marker; globalThis members are untouched.
const scope = globalThis as typeof globalThis & { [marker]?: boolean };
if (!scope[marker]) {
  scope[marker] = true;
  const documentToken = crypto.randomUUID();
  let anchor: SelectionAnchor | null = null;
  // One audio per document: a reading this page started, or a desktop
  // trigger speak (double-copy / hotkey) whose text is this page's selection.
  let owner: ReadingOwner | null = null;
  let trigger: {
    owner: TriggerOwner;
    toCaptured: (start: number, end: number) => [number, number] | null;
  } | null = null;
  let observer: MutationObserver | null = null;
  let panel: HTMLElement | null = null;
  let style: HTMLStyleElement | null = null;
  let pauseButton: HTMLButtonElement | null = null;
  let label: HTMLElement | null = null;
  let paused = false;
  let showPanel = false;
  let siteAllowed = false;
  const hover = paragraphHover(() => {
    void chrome.runtime.sendMessage({ type: "read-paragraph" }).catch(() => cleanup());
  });
  async function loadSettings() {
    const settings = await chrome.storage.local.get(["hoverRead", "showPanel"]);
    // Off unless turned on in settings.
    showPanel = settings.showPanel === true;
    if (panel) panel.hidden = !showPanel;
    hover.setEnabled(siteAllowed && settings.hoverRead === true);
  }
  chrome.storage.onChanged.addListener((_changes, area) => {
    if (area === "local") void loadSettings();
  });
  function refreshSiteAccess() {
    void chrome.runtime
      .sendMessage({ type: "site-access" })
      .then((allowed) => {
        siteAllowed = allowed === true;
        return loadSettings();
      })
      .catch(() => hover.setEnabled(false));
  }
  refreshSiteAccess();
  // Wake the worker on every copy: the double-copy trigger announces within
  // milliseconds of the second copy, and only an awake worker holds the
  // native-host port the announce travels through. One message per copy.
  document.addEventListener("copy", () => {
    void chrome.runtime.sendMessage({ type: "wake" }).catch(() => {});
  });
  const sendControl = (action: ControlAction) => {
    const target = owner?.readingId
      ? { reading_id: owner.readingId }
      : trigger
        ? { trigger_id: trigger.owner.triggerId }
        : null;
    if (target)
      void chrome.runtime
        .sendMessage({ type: "control", document_token: documentToken, ...target, action })
        .catch(() => cleanup());
  };
  /** Stop a reading this page started; a desktop trigger speak outlives the page. */
  const stopReading = () => {
    if (owner) sendControl("stop");
  };
  function cleanup() {
    observer?.disconnect();
    observer = null;
    CSS.highlights?.delete("copyspeak-passage");
    CSS.highlights?.delete("copyspeak-word");
    panel?.remove();
    style?.remove();
    panel = null;
    style = null;
    anchor = null;
    owner = null;
    trigger = null;
  }
  /** Passage highlight plus the playback box, over the current anchor. */
  function present(skip: boolean) {
    if (!anchor?.valid()) {
      stopReading();
      cleanup();
      return;
    }
    const passage = anchor.range(0, anchor.text.length);
    if (!passage) {
      stopReading();
      cleanup();
      return;
    }
    CSS.highlights.set("copyspeak-passage", new Highlight(passage));
    style = document.createElement("style");
    style.textContent =
      "::highlight(copyspeak-passage){background-color:#8cbcff55}::highlight(copyspeak-word){background-color:#1672de;color:#fff}";
    document.documentElement.append(style);
    panel = document.createElement("aside");
    panel.hidden = !showPanel;
    const shadow = panel.attachShadow({ mode: "open" });
    const css = document.createElement("style");
    css.textContent =
      ":host{all:initial;position:fixed;z-index:2147483647;bottom:16px;right:16px}:host([hidden]){display:none}section{display:flex;align-items:center;gap:8px;padding:8px 12px;background:#fff;color:#172232;border:1px solid #8394aa;border-radius:8px;box-shadow:0 2px 12px #17223233;font:13px system-ui}button{font:inherit;color:#172232;background:#edf3fa;border:1px solid #8394aa;border-radius:4px;padding:6px 10px;cursor:pointer}button:focus-visible{outline:3px solid #1672de;outline-offset:2px}";
    const strip = document.createElement("section");
    strip.setAttribute("aria-label", "CopySpeak reading controls");
    label = document.createElement("span");
    label.textContent = "CopySpeak · Buffering";
    label.setAttribute("role", "status");
    const button = (text: string, name: string, onclick: () => void) => {
      const element = document.createElement("button");
      element.textContent = text;
      element.setAttribute("aria-label", name);
      element.onclick = onclick;
      return element;
    };
    pauseButton = button("Pause", "Pause", () => sendControl(paused ? "resume" : "pause"));
    const stop = button("Stop", "Stop", () => {
      sendControl("stop");
      cleanup();
    });
    const hide = button("×", "Hide playback box", () => {
      showPanel = false;
      if (panel) panel.hidden = true;
      void chrome.storage.local.set({ showPanel: false });
    });
    hide.title = "Hide playback box. Restore it in CopySpeak Companion settings.";
    // Skips only when the app takes them: an older native host drops the
    // whole connection on an action it does not know.
    strip.append(label);
    if (skip) strip.append(button("−5s", "Back 5 seconds", () => sendControl("skip-back")));
    strip.append(pauseButton);
    if (skip) strip.append(button("+5s", "Forward 5 seconds", () => sendControl("skip-forward")));
    strip.append(stop, hide);
    shadow.append(css, strip);
    document.documentElement.append(panel);
    anchor.clearIfUnchanged();
    observer = new MutationObserver(() => {
      if (anchor && !anchor.valid()) {
        stopReading();
        cleanup();
      }
    });
    observer.observe(anchor.root, { subtree: true, childList: true, characterData: true });
  }
  /** Reflect one state frame: box label, pause button, and the word span (captured offsets). */
  function show(status: Status, wordAvailable: boolean, word: [number, number] | null) {
    paused = status === "paused";
    if (pauseButton) {
      pauseButton.textContent = paused ? "Resume" : "Pause";
      pauseButton.setAttribute("aria-label", pauseButton.textContent);
    }
    if (label)
      label.textContent = `CopySpeak · ${status}${wordAvailable ? "" : " · Word highlighting unavailable"}`;
    CSS.highlights.delete("copyspeak-word");
    const range = word && anchor?.range(word[0], word[1]);
    if (range) CSS.highlights.set("copyspeak-word", new Highlight(range));
  }
  chrome.runtime.onMessage.addListener((message: CopyCommand, _sender, respond) => {
    if (message.type === "refresh-site-access") {
      refreshSiteAccess();
      return;
    }
    if (message.type === "capture") {
      if (!CSS.highlights || !("Highlight" in globalThis)) {
        respond({ error: "This browser does not support text highlighting." });
        return;
      }
      // Do not read an unfocused frame, including a top frame with a focused iframe.
      if (
        !message.paragraph &&
        document.activeElement?.matches(
          'iframe,frame,input,textarea,[contenteditable]:not([contenteditable="false"])'
        )
      ) {
        respond(null);
        return;
      }
      const captured = message.paragraph ? hover.take() : captureSelection(document);
      if (!captured || captured.text.length > 65536) {
        respond({
          error:
            "Select ordinary webpage text (up to 65,536 UTF-16 units). Editors, PDFs and shadow content are unsupported."
        });
        return;
      }
      sendControl("stop");
      cleanup();
      anchor = captured;
      owner = new ReadingOwner(message.request_id);
      respond({ text: anchor.text, document_token: documentToken });
      return;
    }
    if (message.type === "trigger") {
      // A desktop speak started from copied text. Follow it only when this
      // frame's live selection IS that text; anything else (another app, a
      // cleared or different selection) is a silent no-op.
      const e = parseEvent(message.event);
      if (e?.type !== "trigger-started" || !CSS.highlights || !("Highlight" in globalThis)) return;
      const captured = captureSelection(document);
      const toCaptured =
        captured && captured.text.length <= 65536
          ? matchCopiedText(captured.text, e.raw_text)
          : null;
      if (!captured || !toCaptured) return;
      // The app already cancelled whatever this page was following; drop it
      // locally without a stop, which would stop the new speak instead.
      cleanup();
      anchor = captured;
      const followed = { owner: new TriggerOwner(e.trigger_id), toCaptured };
      trigger = followed;
      present(true);
      void chrome.runtime
        .sendMessage({
          type: "trigger-claim",
          document_token: documentToken,
          trigger_id: e.trigger_id
        })
        .then((claimed) => {
          if (claimed !== true && trigger === followed) cleanup();
        })
        .catch(() => {
          if (trigger === followed) cleanup();
        });
      return;
    }
    if (message.type !== "native" && message.type !== "disconnect") return;
    if (message.type === "disconnect") {
      // Scoped: a reading's disconnect must not clear a trigger this page
      // started following in the meantime, and vice versa.
      const ours =
        message.trigger_id === undefined
          ? owner !== null
          : trigger?.owner.triggerId === message.trigger_id;
      if (ours) cleanup();
      return;
    }
    if (message.document_token !== documentToken) return;
    const e = parseEvent(message.event);
    if (!e) return;
    if (e.type === "state" && "trigger_id" in e) {
      if (!trigger?.owner.apply(e)) return;
      if (trigger.owner.ended || !anchor?.valid()) {
        cleanup();
        return;
      }
      show(e.status, e.word_available, e.word && trigger.toCaptured(e.word.start, e.word.end));
      return;
    }
    if (!owner?.apply(e)) return;
    if (e.type === "accepted") present(e.skip);
    if (e.type === "rejected") cleanup();
    if (e.type === "state") {
      if (owner.ended) {
        cleanup();
        return;
      }
      if (!anchor?.valid()) {
        sendControl("stop");
        cleanup();
        return;
      }
      show(e.status, e.word_available, e.word && [e.word.start, e.word.end]);
    }
  });
  addEventListener("pagehide", () => {
    stopReading();
    cleanup();
  });
}
