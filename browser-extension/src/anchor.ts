export interface SelectionAnchor {
  text: string;
  root: Node;
  valid(): boolean;
  range(start: number, end: number): Range | null;
  clearIfUnchanged(): boolean;
}

/**
 * Match text CopySpeak copied (the clipboard, as the app read it) against a
 * captured selection. The two never agree on whitespace: the clipboard has
 * CRLF on Windows and blank lines between paragraphs, while a capture keeps
 * the page source's whitespace and joins blocks with one "\n". They match
 * when their non-whitespace UTF-16 units are identical, in order. Returns a
 * mapper from a [start, end) span of `copied` (where the app's word offsets
 * live) to the same span of `captured`, or null when the texts differ.
 */
export function matchCopiedText(
  captured: string,
  copied: string
): ((start: number, end: number) => [number, number] | null) | null {
  const space = /\s/;
  const kept: number[] = []; // captured offset of each non-whitespace unit
  for (let i = 0; i < captured.length; i++) if (!space.test(captured.charAt(i))) kept.push(i);
  const ordinal = new Int32Array(copied.length).fill(-1); // copied offset -> index into kept
  let n = 0;
  for (let i = 0; i < copied.length; i++) {
    if (space.test(copied.charAt(i))) continue;
    const at = kept[n];
    if (at === undefined || captured.charCodeAt(at) !== copied.charCodeAt(i)) return null;
    ordinal[i] = n++;
  }
  if (n === 0 || n !== kept.length) return null;
  return (start, end) => {
    if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end > copied.length)
      return null;
    // Spans may carry edge whitespace; their ends are the units they cover.
    while (start < end && ordinal[start] === -1) start++;
    while (end > start && ordinal[end - 1] === -1) end--;
    if (start >= end) return null;
    const first = kept[ordinal[start]!];
    const last = kept[ordinal[end - 1]!];
    return first === undefined || last === undefined ? null : [first, last + 1];
  };
}

/** Offsets are UTF16, as in DOM Range and native CopySpeak captions. */
export function captureSelection(doc: Document, paragraph?: Range): SelectionAnchor | null {
  const selection = doc.getSelection();
  if (!paragraph && (!selection || selection.rangeCount !== 1 || selection.isCollapsed))
    return null;
  const selected = (paragraph ?? selection!.getRangeAt(0)).cloneRange();
  const root = selected.commonAncestorContainer;
  if (root.getRootNode() !== doc || doc.designMode === "on") return null;
  const safe = (node: Node) => {
    let el = node.parentElement;
    while (el) {
      if (
        el.matches(
          'input,textarea,select,script,style,noscript,[hidden],[inert],[contenteditable]:not([contenteditable="false"])'
        ) ||
        el.namespaceURI !== "http://www.w3.org/1999/xhtml"
      )
        return false;
      const style = doc.defaultView!.getComputedStyle(el);
      if (style.display === "none" || style.visibility === "hidden") return false;
      el = el.parentElement;
    }
    return true;
  };
  const parts: { node: Text; from: number; to: number; start: number; value: string }[] = [];
  const walker = doc.createTreeWalker(root, 5);
  let node: Node | null = root.nodeType === 3 ? root : walker.nextNode();
  let text = "";
  let previousBlock: Element | null = null;
  let lineBreak = false;
  const block = (node: Node) => {
    let el = node.parentElement;
    while (el && el !== doc.documentElement) {
      if (
        /^(block|list-item|table-row|table-cell|flex|grid|flow-root)$/.test(
          doc.defaultView!.getComputedStyle(el).display
        )
      )
        return el;
      el = el.parentElement;
    }
    return el;
  };
  while (node) {
    // SAFETY: nodeType === 1 exhausts Element nodes, so node is an Element here.
    if (node.nodeType === 1 && (node as Element).tagName === "BR" && selected.intersectsNode(node))
      lineBreak = true;
    // SAFETY: nodeType === 3 exhausts text nodes, so node is a Text node here.
    // Hidden, script, SVG and editable text is skipped, not fatal: page
    // selections routinely cross icons, inline scripts and hidden labels.
    // A selection made only of such text still yields nothing and is refused.
    if (node.nodeType === 3 && selected.intersectsNode(node) && safe(node)) {
      // SAFETY: inside the nodeType === 3 branch, node is a Text node (see comment above).
      const value = (node as Text).data;
      const from = node === selected.startContainer ? selected.startOffset : 0;
      const to = node === selected.endContainer ? selected.endOffset : value.length;
      if (to > from) {
        const currentBlock = block(node);
        if (text && (lineBreak || currentBlock !== previousBlock)) text += "\n";
        lineBreak = false;
        previousBlock = currentBlock;
        // SAFETY: same text-node branch types node as Text (nodeType === 3 above).
        parts.push({ node: node as Text, from, to, start: text.length, value });
        text += value.slice(from, to);
      }
    }
    node = walker.nextNode();
  }
  if (!text.trim()) return null;
  const original = selected.toString();
  const valid = () =>
    parts.every((p) => p.node.isConnected && p.node.data === p.value) &&
    selected.toString() === original;
  const boundary = (n: number) =>
    !(
      text.charCodeAt(n - 1) >= 0xd800 &&
      text.charCodeAt(n - 1) <= 0xdbff &&
      text.charCodeAt(n) >= 0xdc00 &&
      text.charCodeAt(n) <= 0xdfff
    );
  return {
    text,
    root,
    valid,
    range(start, end) {
      if (
        !valid() ||
        !Number.isInteger(start) ||
        !Number.isInteger(end) ||
        start < 0 ||
        end <= start ||
        end > text.length ||
        !boundary(start) ||
        !boundary(end)
      )
        return null;
      const first = parts.find((p) => start >= p.start && start < p.start + p.to - p.from);
      const last = parts.find((p) => end > p.start && end <= p.start + p.to - p.from);
      if (!first || !last) return null;
      const range = doc.createRange();
      range.setStart(first.node, first.from + start - first.start);
      range.setEnd(last.node, last.from + end - last.start);
      return range;
    },
    clearIfUnchanged() {
      if (paragraph || !valid() || !selection || selection.rangeCount !== 1) return false;
      const now = selection.getRangeAt(0);
      if (
        now.startContainer !== selected.startContainer ||
        now.startOffset !== selected.startOffset ||
        now.endContainer !== selected.endContainer ||
        now.endOffset !== selected.endOffset
      )
        return false;
      selection.removeAllRanges();
      return true;
    }
  };
}
