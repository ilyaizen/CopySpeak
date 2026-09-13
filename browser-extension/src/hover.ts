import { captureSelection, type SelectionAnchor } from "./anchor";

// The app icon doubles as the hover button; pages may only load it once it is
// listed under web_accessible_resources in the manifest.
const ICON_URL = chrome.runtime.getURL("icons/64.png");

/** One delegated listener; page text and the user's selection stay untouched. */
export function paragraphHover(read: () => void) {
  let enabled = false;
  let paragraph: Element | null = null;
  let pending: SelectionAnchor | null = null;
  let host: HTMLElement | null = null;
  let highlightStyle: HTMLStyleElement | null = null;
  function clear() {
    paragraph = null;
    host?.remove();
    host = null;
    highlightStyle?.remove();
    highlightStyle = null;
    CSS.highlights?.delete("copyspeak-hover");
  }
  function show(target: Element) {
    clear();
    const range = document.createRange();
    range.selectNodeContents(target);
    const captured = captureSelection(document, range);
    if (!captured || captured.text.length > 65536) return;
    paragraph = target;
    CSS.highlights.set("copyspeak-hover", new Highlight(range));
    highlightStyle = document.createElement("style");
    highlightStyle.textContent = "::highlight(copyspeak-hover){background-color:#8cbcff22}";
    document.documentElement.append(highlightStyle);
    host = document.createElement("aside");
    const shadow = host.attachShadow({ mode: "open" });
    const css = document.createElement("style");
    const rect = target.getBoundingClientRect();
    css.textContent = `:host{all:initial;position:fixed;z-index:2147483647;left:${Math.max(4, Math.min(rect.right - 30, innerWidth - 36))}px;top:${Math.max(4, Math.min(rect.top, innerHeight - 36))}px}button{padding:0;width:30px;height:30px;border:none;border-radius:8px;background:transparent;box-shadow:0 2px 8px #17223233;cursor:pointer;transition:transform .12s ease,box-shadow .12s ease}button:hover{transform:scale(1.1);box-shadow:0 3px 10px #1722324d}button:focus-visible{outline:3px solid #1672de;outline-offset:2px}button img{display:block;width:30px;height:30px;border-radius:8px}`;
    const button = document.createElement("button");
    const icon = document.createElement("img");
    icon.src = ICON_URL;
    icon.alt = "";
    button.append(icon);
    button.title = "Read paragraph with CopySpeak";
    button.setAttribute("aria-label", button.title);
    button.onpointerdown = (event) => event.preventDefault();
    button.onclick = () => {
      // Capture at click time: dynamic pages can change text since hover began.
      if (!paragraph?.isConnected) return clear();
      const current = document.createRange();
      current.selectNodeContents(paragraph);
      pending = captureSelection(document, current);
      if (!pending || pending.text.length > 65536) return clear();
      clear();
      read();
    };
    shadow.append(css, button);
    document.documentElement.append(host);
  }
  function move(event: PointerEvent) {
    if (
      !enabled ||
      event.pointerType === "touch" ||
      event.buttons ||
      !CSS.highlights ||
      !("Highlight" in globalThis)
    )
      return;
    if (!(event.target instanceof Element) || event.composedPath().includes(host!)) return;
    if (host?.shadowRoot?.activeElement) return;
    const target = event.target.closest("p,li,blockquote");
    if (
      !target ||
      event.target.closest(
        'a,button,input,textarea,select,[role="button"],[contenteditable]:not([contenteditable="false"]),[inert],[hidden]'
      )
    )
      return clear();
    if (target === paragraph) return;
    show(target);
  }
  function leave(event: PointerEvent) {
    if (!event.relatedTarget) clear();
  }
  function escape(event: KeyboardEvent) {
    if (event.key === "Escape") clear();
  }
  return {
    setEnabled(value: boolean) {
      if (enabled === value) return;
      enabled = value;
      if (value) {
        document.addEventListener("pointermove", move);
        document.addEventListener("pointerout", leave);
        document.addEventListener("scroll", clear, true);
        document.addEventListener("keydown", escape);
        addEventListener("resize", clear);
        addEventListener("pagehide", clear);
      } else {
        document.removeEventListener("pointermove", move);
        document.removeEventListener("pointerout", leave);
        document.removeEventListener("scroll", clear, true);
        document.removeEventListener("keydown", escape);
        removeEventListener("resize", clear);
        removeEventListener("pagehide", clear);
        pending = null;
        clear();
      }
    },
    take() {
      const captured = pending;
      pending = null;
      return captured?.valid() ? captured : null;
    }
  };
}
