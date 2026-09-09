import { describe, test, expect } from 'bun:test';
import { JSDOM } from 'jsdom';
import { captureSelection } from '../src/anchor';

function fixture(html: string, first: string, start: number, last: string, end: number) {
  const dom = new JSDOM(`<main>${html}</main>`);
  const d = dom.window.document;
  const range = d.createRange();
  range.setStart(d.querySelector(first)!.firstChild!, start);
  range.setEnd(d.querySelector(last)!.firstChild!, end);
  const selection = d.getSelection()!;
  selection.addRange(range);
  return { d, selection };
}

describe('selection anchor', () => {
  test('invalidates inserted source but preserves a newer native selection', () => {
    const {d, selection} = fixture('<p>one <b>two</b> three</p>', 'p', 0, 'b', 3);
    const anchor = captureSelection(d)!;
    const newer = d.createRange(); newer.selectNodeContents(d.querySelector('b')!);
    selection.removeAllRanges(); selection.addRange(newer);
    expect(anchor.clearIfUnchanged()).toBe(false);
    expect(selection.toString()).toBe('two');
    d.querySelector('b')!.before(d.createTextNode('insert'));
    expect(anchor.valid()).toBe(false);
    expect(anchor.range(0, 3) === null).toBe(true);
  });
  test('refuses editable or hidden content and shadow selections', () => {
    for (const html of ['<p contenteditable="true">secret</p>', '<p hidden>secret</p>', '<p style="display:none">secret</p>', '<textarea>secret</textarea>']) {
      const {d} = fixture(html, 'main > *', 0, 'main > *', 6);
      expect(captureSelection(d)).toBeNull();
    }
  });
  test('inserts unmapped block and BR separators while preserving inline continuity', () => {
    const {d} = fixture('<p>one<br>two</p><p><b>thr</b>ee</p>', 'p', 0, 'b', 3);
    const anchor = captureSelection(d)!;
    expect(anchor.text).toBe('one\ntwo\nthr');
    expect(anchor.range(3, 4)).toBeNull();
    expect(anchor.range(8, 11)?.toString()).toBe('thr');
  });
  test('clips UTF16 source across inline nodes without locating repeated words', () => {
    const {d, selection} = fixture('<p>skip 😀 same <b>same</b> tail</p>', 'p', 5, 'b', 4);
    const anchor = captureSelection(d)!;
    expect(anchor.text).toBe('😀 same same');
    expect(anchor.range(8, 12)?.toString()).toBe('same');
    expect(anchor.range(1, 2)).toBeNull(); // inside surrogate
    expect(anchor.clearIfUnchanged()).toBe(true);
    expect(selection.rangeCount).toBe(0);
  });
});
