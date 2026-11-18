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
  let dot: HTMLElement | null = null;
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
  // Glyphs match the DSH playback bar (client.js PauseGlyph, PlayGlyph,
  // StopGlyph, SkipGlyph). Back and forward are the Material fast_rewind and
  // fast_forward paths. Each part is one SVG child element.
  type Glyph = "pause" | "play" | "stop" | "back" | "forward" | "close";
  type GlyphPart = { tag: "rect" | "path"; attrs: Record<string, string> };
  const GLYPHS = {
    pause: {
      viewBox: "0 0 16 16",
      parts: [
        {
          tag: "rect",
          attrs: { x: "4.6", y: "3", width: "2.5", height: "10", rx: "0.9", fill: "currentColor" }
        },
        {
          tag: "rect",
          attrs: { x: "8.9", y: "3", width: "2.5", height: "10", rx: "0.9", fill: "currentColor" }
        }
      ]
    },
    play: {
      viewBox: "0 0 16 16",
      parts: [
        {
          tag: "path",
          attrs: {
            d: "M5.8 3.5 12.2 8 5.8 12.5Z",
            fill: "currentColor",
            stroke: "currentColor",
            "stroke-width": "1",
            "stroke-linejoin": "round"
          }
        }
      ]
    },
    stop: {
      viewBox: "0 0 16 16",
      parts: [
        {
          tag: "rect",
          attrs: {
            x: "3.8",
            y: "3.8",
            width: "8.4",
            height: "8.4",
            rx: "1.3",
            fill: "currentColor"
          }
        }
      ]
    },
    back: {
      viewBox: "0 0 24 24",
      parts: [
        {
          tag: "path",
          attrs: { d: "M11 18V6l-8.5 6 8.5 6zm.5-6l8.5 6V6l-8.5 6z", fill: "currentColor" }
        }
      ]
    },
    forward: {
      viewBox: "0 0 24 24",
      parts: [
        {
          tag: "path",
          attrs: { d: "M4 18l8.5-6L4 6v12zm9-12v12l8.5-6L13 6z", fill: "currentColor" }
        }
      ]
    },
    close: {
      viewBox: "0 0 16 16",
      parts: [
        {
          tag: "path",
          attrs: {
            d: "M3 3l10 10M13 3 3 13",
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "2",
            "stroke-linecap": "round"
          }
        }
      ]
    }
  } satisfies Record<Glyph, { viewBox: string; parts: GlyphPart[] }>;
  /** One glyph as an SVG element, built from the GLYPHS table. */
  function glyphElement(name: Glyph): SVGSVGElement {
    const glyph = GLYPHS[name];
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", glyph.viewBox);
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");
    for (const part of glyph.parts) {
      const element = document.createElementNS("http://www.w3.org/2000/svg", part.tag);
      for (const [key, value] of Object.entries(part.attrs)) element.setAttribute(key, value);
      svg.append(element);
    }
    return svg;
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
    // Icon-only bar, like the DSH playback bar: a floating pill of 26 px
    // controls. System colors follow the page's light or dark scheme.
    css.textContent = [
      ":host{all:initial;position:fixed;z-index:2147483647;bottom:20px;right:20px}",
      ":host([hidden]){display:none}",
      ".bar{display:inline-flex;align-items:center;gap:2px;box-sizing:border-box;max-width:calc(100vw - 40px);padding:6px;border:.5px solid rgb(127 127 127 / 40%);border-radius:12px;background:Canvas;color:CanvasText;box-shadow:0 6px 20px rgb(0 0 0 / 14%);font:13px system-ui}",
      ".control{flex:none;display:inline-flex;align-items:center;justify-content:center;width:26px;height:26px;padding:0;border:none;border-radius:6px;background:none;color:inherit;cursor:pointer}",
      "@media (hover:hover){.control:hover{background:rgb(127 127 127 / 16%)}}",
      ".control:focus-visible{outline:2px solid #1672de;outline-offset:2px}",
      ".control svg{width:16px;height:16px;fill:currentColor;pointer-events:none}",
      ".close{width:22px;height:22px;opacity:.7}",
      ".close svg{width:12px;height:12px}",
      ".dot{flex:none;width:7px;height:7px;margin:0 6px 0 4px;border-radius:50%;background:#8394aa}",
      '.dot[data-state="playing"]{background:#1672de}',
      '.dot[data-state="buffering"]{background:#e0a100;animation:pulse 1s ease-in-out infinite}',
      "@keyframes pulse{50%{opacity:.35}}",
      "@media (prefers-reduced-motion:reduce){.dot{animation:none!important}}",
      ".sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}"
    ].join("");
    const strip = document.createElement("section");
    strip.className = "bar";
    strip.setAttribute("aria-label", "CopySpeech reading controls");
    dot = document.createElement("span");
    dot.className = "dot";
    dot.setAttribute("aria-hidden", "true");
    // Screen readers hear the state; sighted users see the dot.
    label = document.createElement("span");
    label.className = "sr";
    label.textContent = "CopySpeech · Buffering";
    label.setAttribute("role", "status");
    const control = (glyph: Glyph, name: string, onclick: () => void, extra = "") => {
      const element = document.createElement("button");
      element.type = "button";
      element.className = extra ? `control ${extra}` : "control";
      element.title = name;
      element.setAttribute("aria-label", name);
      element.append(glyphElement(glyph));
      element.onclick = onclick;
      return element;
    };
    pauseButton = control("pause", "Pause", () => sendControl(paused ? "resume" : "pause"));
    const stop = control("stop", "Stop", () => {
      sendControl("stop");
      cleanup();
    });
    const hide = control(
      "close",
      "Hide playback box",
      () => {
        showPanel = false;
        if (panel) panel.hidden = true;
        void chrome.storage.local.set({ showPanel: false });
      },
      "close"
    );
    hide.title = "Hide playback box. Restore it in CopySpeech Companion settings.";
    // Skips only when the app takes them: an older native host drops the
    // whole connection on an action it does not know.
    strip.append(label, dot);
    if (skip) strip.append(control("back", "Back 5 seconds", () => sendControl("skip-back")));
    strip.append(pauseButton);
    if (skip)
      strip.append(control("forward", "Forward 5 seconds", () => sendControl("skip-forward")));
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
      const name = paused ? "Resume" : "Pause";
      pauseButton.replaceChildren(glyphElement(paused ? "play" : "pause"));
      pauseButton.title = name;
      pauseButton.setAttribute("aria-label", name);
    }
    if (dot) dot.dataset.state = status;
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
