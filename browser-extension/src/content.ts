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
import { CaptionGlide } from "./glide";

// Repeated scripting.executeScript calls do not install duplicate listeners.
const marker = "__copyspeech_companion_v1__";
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
  let pauseButton: HTMLButtonElement | null = null;
  let label: HTMLElement | null = null;
  let glide: CaptionGlide | null = null;
  let paused = false;
  let showPanel = false;
  // The caption pill glides until turned off in settings (DSH default).
  let captionAnimation = true;
  let siteAllowed = false;
  const hover = paragraphHover(() => {
    void chrome.runtime.sendMessage({ type: "read-paragraph" }).catch(() => cleanup());
  });
  async function loadSettings() {
    const settings = await chrome.storage.local.get(["hoverRead", "showPanel", "captionAnimation"]);
    // Off unless turned on in settings.
    showPanel = settings.showPanel === true;
    if (panel) panel.hidden = !showPanel;
    // On unless turned off, so captions animate like the DSH plugin's.
    captionAnimation = settings.captionAnimation !== false;
    glide?.setAnimated(captionAnimation);
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
  // The pill follows page reflows it cannot own: content mutations go
  // through the reading's observer, resizes and box scrolls through this listener.
  function repositionOnResize() {
    glide?.reposition();
  }
  // Follow scroll: the spoken line stays inside this band of its scroll port.
  // A word outside the band scrolls the port so the word sits on the anchor line.
  const FOLLOW_BAND_TOP = 0.15;
  const FOLLOW_BAND_BOTTOM = 0.82;
  const FOLLOW_ANCHOR = 0.34;
  // A user scroll gesture owns the view for a moment, so following waits.
  const FOLLOW_PAUSE_MS = 5000;
  const FOLLOW_INTERVAL_MS = 400;
  const FOLLOW_KEYS = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"]);
  let followPort: Element | null = null;
  let followPausedUntil = 0;
  let lastFollowAt = 0;
  let paintedWord: string | null = null;
  /** The nearest scrollable ancestor, or null when the page itself scrolls. */
  function scrollPort(node: Node): Element | null {
    let element = node instanceof Element ? node : node.parentElement;
    while (element !== null && element !== document.documentElement) {
      const overflow = getComputedStyle(element).overflowY;
      if (
        (overflow === "auto" || overflow === "scroll") &&
        element.scrollHeight > element.clientHeight
      )
        return element;
      element = element.parentElement;
    }
    return null;
  }
  const pauseFollow = () => {
    followPausedUntil = Date.now() + FOLLOW_PAUSE_MS;
  };
  const pauseFollowKey = (event: KeyboardEvent) => {
    if (FOLLOW_KEYS.has(event.key)) pauseFollow();
  };
  function startFollowing(root: Node) {
    followPort = scrollPort(root);
    const target: EventTarget = followPort ?? window;
    target.addEventListener("wheel", pauseFollow, { passive: true });
    target.addEventListener("touchstart", pauseFollow, { passive: true });
    addEventListener("keydown", pauseFollowKey, true);
    // A box scroll moves the word without moving the document: re-place the pill.
    followPort?.addEventListener("scroll", repositionOnResize, { passive: true });
  }
  function stopFollowing() {
    const target: EventTarget = followPort ?? window;
    target.removeEventListener("wheel", pauseFollow);
    target.removeEventListener("touchstart", pauseFollow);
    removeEventListener("keydown", pauseFollowKey, true);
    followPort?.removeEventListener("scroll", repositionOnResize);
    followPort = null;
    followPausedUntil = 0;
    lastFollowAt = 0;
  }
  /** Scroll the spoken word back into the band, unless it is already there. */
  function followWord(range: Range) {
    const view = document.defaultView;
    if (view === null) return;
    const now = Date.now();
    if (now < followPausedUntil || now - lastFollowAt < FOLLOW_INTERVAL_MS) return;
    const rect = range.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return;
    const boxTop = followPort === null ? 0 : followPort.getBoundingClientRect().top;
    const height = followPort === null ? view.innerHeight : followPort.clientHeight;
    if (height === 0) return;
    const top = rect.top - boxTop;
    const bottom = rect.bottom - boxTop;
    if (top >= height * FOLLOW_BAND_TOP && bottom <= height * FOLLOW_BAND_BOTTOM) return;
    const reduced =
      view.matchMedia !== undefined && view.matchMedia("(prefers-reduced-motion: reduce)").matches;
    lastFollowAt = now;
    (followPort ?? view).scrollBy({
      top: top - height * FOLLOW_ANCHOR,
      behavior: reduced ? "auto" : "smooth"
    });
  }
  function cleanup() {
    observer?.disconnect();
    observer = null;
    removeEventListener("resize", repositionOnResize);
    stopFollowing();
    glide?.release();
    glide = null;
    panel?.remove();
    panel = null;
    paintedWord = null;
    anchor = null;
    owner = null;
    trigger = null;
  }
  /** The playback box, controls and follow scroll, over the current anchor. */
  function present(skip: boolean) {
    if (!anchor?.valid()) {
      stopReading();
      cleanup();
      return;
    }
    glide = new CaptionGlide(document, captionAnimation);
    paintedWord = null;
    startFollowing(anchor.root);
    addEventListener("resize", repositionOnResize);
    panel = document.createElement("aside");
    panel.hidden = !showPanel;
    const shadow = panel.attachShadow({ mode: "open" });
    const css = document.createElement("style");
    css.textContent =
      ":host{all:initial;position:fixed;z-index:2147483647;bottom:16px;right:16px}:host([hidden]){display:none}section{display:flex;align-items:center;gap:8px;padding:8px 12px;background:#fff;color:#172232;border:1px solid #8394aa;border-radius:8px;box-shadow:0 2px 12px #17223233;font:13px system-ui}button{font:inherit;color:#172232;background:#edf3fa;border:1px solid #8394aa;border-radius:4px;padding:6px 10px;cursor:pointer}button:focus-visible{outline:3px solid #1672de;outline-offset:2px}";
    const strip = document.createElement("section");
    strip.setAttribute("aria-label", "CopySpeech reading controls");
    label = document.createElement("span");
    label.textContent = "CopySpeech · Buffering";
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
    hide.title = "Hide playback box. Restore it in CopySpeech Companion settings.";
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
        return;
      }
      // The reading's text survived: keep the pill on the spoken word.
      glide?.reposition();
    });
    observer.observe(anchor.root, { subtree: true, childList: true, characterData: true });
  }
  /** Reflect one state frame: box label, pause button, and the word pill (captured offsets). */
  function show(status: Status, wordAvailable: boolean, word: [number, number] | null) {
    paused = status === "paused";
    if (pauseButton) {
      pauseButton.textContent = paused ? "Resume" : "Pause";
      pauseButton.setAttribute("aria-label", pauseButton.textContent);
    }
    if (label)
      label.textContent = `CopySpeech · ${status}${wordAvailable ? "" : " · Word highlighting unavailable"}`;
    if (word === null || glide === null || anchor === null) {
      // A wordless state frame (caption gap, trailing fragment audio, a
      // degraded fragment) holds the last paint: the native side keeps the
      // playing status through inter-word gaps and fragment tails, and a
      // teardown here makes the NEXT word settle instantly (the reported
      // abrupt word-swap). A reading boundary tears down through cleanup().
      glide?.hold();
      return;
    }
    const range = anchor.range(word[0], word[1]);
    if (range === null) {
      // The word did not map onto the page: hold the previous paint instead
      // of tearing the glide down for one unmappable frame.
      glide.hold();
      return;
    }
    const key = `${word[0]}:${word[1]}`;
    if (key !== paintedWord) {
      paintedWord = key;
      followWord(range);
    }
    glide.paint(range, key);
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
