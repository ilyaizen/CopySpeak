export interface SelectionAnchor {
  text: string;
  root: Node;
  valid(): boolean;
  range(start: number, end: number): Range | null;
  clearIfUnchanged(): boolean;
}

/** Offsets are UTF16, as in DOM Range and native CopySpeak captions. */
export function captureSelection(doc: Document): SelectionAnchor | null {
  const selection = doc.getSelection();
  if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) return null;
  const selected = selection.getRangeAt(0).cloneRange();
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
      if (!valid() || selection.rangeCount !== 1) return false;
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
