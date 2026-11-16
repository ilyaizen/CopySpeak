/*
 * Regression driver for the DSH plugin caption pill.
 *
 * Loads the real copyspeech-dsh/client.js into a stub-host page, fakes the
 * CopySpeech control server on 127.0.0.1:43117, starts one message reading, and
 * asserts the difference-blend pill paints over the spoken word. Further checks
 * cover a scrolled document, a scrolled chat scrollport, and a scrolled window.
 *
 * Run from the repo root:  bun scripts/test-dsh-plugin-captions.mjs
 * CAPTION_CLIENT=<path> checks another client.js, so the checks can be proven
 * to fail on an older revision.
 */
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const clientPath = process.env.CAPTION_CLIENT ?? join(here, "..", "copyspeech-dsh", "client.js");
const clientSrc = readFileSync(clientPath, "utf8");

/* The reading entry point sits inside the bundle closure. Expose it to the
   fixture the way the host's message action would call it, without touching
   the shipped file. */
const HOOK_ANCHOR = "    function beginReading(messageId, nodeKey, speech, range = null) {";
if (!clientSrc.includes(HOOK_ANCHOR)) {
  throw new Error("test hook anchor not found in " + clientPath);
}
const servedClient = clientSrc.replace(
  HOOK_ANCHOR,
  [
    "    window.__csTestBegin = function (messageId, nodeKey, speech) {",
    "      return beginReading(messageId, nodeKey, speech);",
    "    };",
    HOOK_ANCHOR
  ].join("\n")
);

const NODE_KEY = "caption-test-row";
const SPEECH =
  "The caption pill glides from word to word while the reply plays aloud over a long line";
const WORD_MS = 400;
const words = [];
for (const match of SPEECH.matchAll(/\S+/gu)) {
  words.push({ text_start: match.index, text_end: match.index + match[0].length });
}
const totalMs = words.length * WORD_MS;

const stub = `
window.__ModuleLoader__ = {
  load(def) {
    const reactStub = {
      createElement: function (type, props) {
        return { type: type, props: props || {} };
      }
    };
    const mod = def.factory(function (name) {
      if (name === "react") return reactStub;
      throw new Error("no stub for " + name);
    });
    mod.apply({
      effect: function (fn) { return fn(); },
      locale: {
        register: function () { return function () {}; },
        bind: function () { return function (key) { return key; }; }
      },
      slots: {
        inject: function () {},
        register: function () { return function () {}; }
      },
      sessions: { binding: function () { return undefined; } },
      uiConversation: { interactions: { register: function () { return function () {}; } } }
    });
  }
};
`;

function fixtureHtml() {
  return `<!doctype html>
<html><head><style>
body { margin: 0; padding: 48px; background: #101010; color: #e8e8e8; font: 16px/1.7 system-ui, sans-serif; }
.reply { max-width: 640px; }
#spacer { height: 0; }
/* The chat scrollport: the real host scrolls an inner element, not the page. */
#scrollbox { height: 300px; overflow-y: auto; }
#filler { height: 800px; }
#tail { height: 1400px; }
</style>
<script>${stub}</${"script"}>
<script src="/client.js"></${"script"}>
</head><body>
<div id="spacer"></div>
<div id="scrollbox">
  <div class="reply" data-chat-flow-kind="assistant-step" data-chat-node-key="${NODE_KEY}">
    <div data-text-interaction-source><p id="line">${SPEECH}</p></div>
  </div>
  <div id="filler"></div>
</div>
<div id="tail"></div>
</body></html>`;
}

/* Failures accumulate so one run reports every broken check. */
const failures = [];
function check(condition, message) {
  if (condition) {
    console.log("ok   " + message);
    return true;
  }
  failures.push(message);
  console.log("FAIL " + message);
  return false;
}

function createServer() {
  const state = {
    startedAt: null,
    freeze: null,
    gate: Promise.resolve(),
    resolveGate: () => {}
  };
  function statusBody() {
    if (state.startedAt === null) return { active: false, paused: false };
    const position =
      state.freeze !== null ? state.freeze : Math.max(0, Date.now() - state.startedAt);
    const active = state.freeze !== null || position < totalMs;
    return {
      active: active,
      paused: false,
      position_ms: position,
      fragment_index: 0,
      captions: {
        text: SPEECH,
        words: words.map((word, index) => ({
          text_start: word.text_start,
          text_end: word.text_end,
          start_ms: index * WORD_MS,
          end_ms: (index + 1) * WORD_MS
        }))
      }
    };
  }
  async function handle(route) {
    const url = new URL(route.request().url());
    const path = url.pathname;
    if (path === "/health") return route.fulfill({ status: 200, body: "ok" });
    if (path === "/status") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { "access-control-allow-origin": "*" },
        body: JSON.stringify(statusBody())
      });
    }
    if (path === "/speak") {
      state.startedAt = Date.now();
      state.gate = new Promise((resolve) => {
        state.resolveGate = resolve;
      });
      await state.gate;
      state.startedAt = null;
      return route.fulfill({ status: 200, body: "" });
    }
    return route.fulfill({ status: 200, body: "" });
  }
  return { state, handle };
}

async function openFixture(browser, server, options = {}) {
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  const errors = [];
  page.on("pageerror", (error) => {
    errors.push(error.message);
    /* The poll throws on every tick, so print each distinct message once. */
    if (errors.indexOf(error.message) === errors.length - 1) {
      console.log("[pageerror] " + error.message);
    }
  });
  page.on("console", (message) => {
    if (message.type() === "error") console.log("[console] " + message.text());
  });
  await page.addInitScript((follow) => {
    try {
      localStorage.setItem("dsh.copyspeech-voice.captions", "1");
      if (!follow) localStorage.setItem("dsh.copyspeech-voice.follow", "0");
    } catch {
      /* about:blank has no storage. */
    }
  }, options.follow !== false);
  await page.route("**/fixture.local/**", (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === "/client.js") {
      return route.fulfill({ status: 200, contentType: "text/javascript", body: servedClient });
    }
    return route.fulfill({ status: 200, contentType: "text/html", body: fixtureHtml() });
  });
  await page.route("**/127.0.0.1:43117/**", server.handle);
  await page.goto("http://fixture.local/", { waitUntil: "load" });
  await page.waitForFunction(() => window.__csTestBegin !== undefined, null, { timeout: 5000 });
  return { page, errors };
}

/* Word rects straight from the rendered text, the same geometry the pill must
   cover. */
function readWordRects(page) {
  return page.evaluate(() => {
    const line = document.getElementById("line");
    const node = line === null ? null : line.firstChild;
    if (node === null || node.nodeType !== Node.TEXT_NODE) throw new Error("no reply text node");
    const out = [];
    const re = /\S+/gu;
    let match;
    while ((match = re.exec(node.textContent)) !== null) {
      const range = document.createRange();
      range.setStart(node, match.index);
      range.setEnd(node, match.index + match[0].length);
      const rect = range.getBoundingClientRect();
      out.push({
        text: match[0],
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height
      });
    }
    return out;
  });
}

function readMark(page) {
  return page.evaluate(() => {
    const mark = document.documentElement.querySelector(".dsh-csva-mark");
    if (mark === null) return null;
    const computed = getComputedStyle(mark);
    const rect = mark.getBoundingClientRect();
    return {
      connected: mark.isConnected,
      visible: mark.checkVisibility(),
      position: computed.position,
      blend: computed.mixBlendMode,
      background: computed.backgroundColor,
      zIndex: computed.zIndex,
      left: parseFloat(mark.style.left),
      top: parseFloat(mark.style.top),
      width: parseFloat(mark.style.width),
      height: parseFloat(mark.style.height),
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
    };
  });
}

function startReading(page) {
  return page.evaluate((speech) => {
    void window.__csTestBegin(1, "caption-test-row", speech);
  }, SPEECH);
}

function settledOn(mark, word) {
  return (
    Math.abs(mark.left - (word.left - 4)) <= 1.5 &&
    Math.abs(mark.top - (word.top - 2)) <= 1.5 &&
    Math.abs(mark.width - (word.width + 8)) <= 1.5 &&
    Math.abs(mark.height - (word.height + 4)) <= 1.5
  );
}

/* A sample can land mid-glide: the position then belongs to the incoming
 * word, while the width is still between the outgoing and incoming widths. */
function glidesBetween(mark, words, active) {
  const candidates = words.slice(Math.max(0, active - 2), Math.min(words.length, active + 2));
  const placed = candidates.some(
    (word) =>
      Math.abs(mark.left - (word.left - 4)) <= 1.5 &&
      Math.abs(mark.top - (word.top - 2)) <= 1.5 &&
      Math.abs(mark.height - (word.height + 4)) <= 1.5
  );
  if (!placed) return false;
  const widths = candidates.map((word) => word.width + 8);
  return mark.width >= Math.min(...widths) - 1.5 && mark.width <= Math.max(...widths) + 1.5;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function movingReading(browser) {
  console.log("\n# A reading that advances: the pill follows the spoken word");
  const server = createServer();
  const { page, errors } = await openFixture(browser, server);
  const rects = await readWordRects(page);
  check(rects.length === words.length, "fixture renders every spoken word");

  await startReading(page);
  for (let wait = 0; wait < 40; wait += 1) {
    if (await readMark(page)) break;
    await sleep(100);
  }
  const first = await readMark(page);
  /* The first painted word proves the fake /speak set the clock. */
  const startedAt = server.state.startedAt;
  if (
    !check(first !== null, "a pill exists under document.documentElement while the reply plays")
  ) {
    console.log("     page errors: " + JSON.stringify([...new Set(errors)]));
    await page.close();
    return;
  }
  check(first.connected === true, "the pill stays connected");
  check(first.visible === true, "the pill is visible");
  check(first.position === "fixed", "the pill is position: fixed");
  check(first.blend === "difference", "the pill uses mix-blend-mode: difference");
  check(first.background === "rgb(255, 255, 255)", "the pill has a white fill");
  check(first.zIndex === "2147483646", "the pill paints above the host UI");
  check(startedAt !== null, "the fake /speak started the clock");

  let matched = 0;
  let samples = 0;
  let misplaced = null;
  while (samples < 14) {
    const mark = await readMark(page);
    if (mark === null) break;
    samples += 1;
    const position = Date.now() - startedAt;
    const active = Math.floor(position / WORD_MS);
    if (glidesBetween(mark, rects, active)) matched += 1;
    else if (misplaced === null) misplaced = { position, mark, active };
    await sleep(120);
  }
  check(samples >= 10, "the pill painted through a run of words (" + samples + " samples)");
  check(
    matched === samples,
    "every sample sits on the spoken word (" + matched + "/" + samples + ")"
  );
  if (misplaced !== null) {
    console.log("     misplaced sample: " + JSON.stringify(misplaced));
  }
  check(errors.length === 0, "no page error during the reading");

  server.state.resolveGate();
  let cleared = false;
  for (let wait = 0; wait < 20; wait += 1) {
    if ((await readMark(page)) === null) {
      cleared = true;
      break;
    }
    await sleep(100);
  }
  check(cleared, "the pill is removed when the reading stops");
  await page.close();
}

async function scrolledReading(browser) {
  console.log("\n# A scrolled document: the fixed pill uses viewport coordinates");
  const server = createServer();
  const { page, errors } = await openFixture(browser, server);
  await page.evaluate(() => {
    document.getElementById("spacer").style.height = "400px";
    window.scrollTo(0, 380);
  });
  await sleep(100);
  const scrolled = await page.evaluate(() => window.scrollY);
  check(scrolled > 300, "the fixture document scrolled (" + scrolled + " px)");

  const rects = await readWordRects(page);
  const target = 6;
  /* Freeze the clock inside one word, so the pill settles before the check. */
  server.state.freeze = target * WORD_MS + 20;
  await startReading(page);
  for (let wait = 0; wait < 40; wait += 1) {
    if (await readMark(page)) break;
    await sleep(100);
  }
  const mark = await readMark(page);
  if (
    !check(
      mark !== null,
      "a pill exists under document.documentElement after the document scrolled"
    )
  ) {
    console.log("     page errors: " + JSON.stringify([...new Set(errors)]));
    await page.close();
    return;
  }
  const word = rects[target];
  check(mark.top <= 600 && mark.top >= 0, "the frozen word is inside the viewport");
  check(settledOn(mark, word), "the pill covers the frozen word at its viewport rect");
  check(
    Math.abs(mark.rect.top - mark.top) <= 1 && Math.abs(mark.rect.left - mark.left) <= 1,
    "the settled pill paints where its offset says"
  );
  if (!settledOn(mark, word)) {
    console.log(
      "     pill " +
        JSON.stringify({ left: mark.left, top: mark.top, width: mark.width }) +
        " word " +
        JSON.stringify(word)
    );
  }
  check(errors.length === 0, "no page error during the scrolled reading");
  server.state.resolveGate();
  await page.close();
}

/* A fixed pill must re-read the word after the chat scrollport or the window
 * moves. Follow is off here, so the follow scroll cannot hide a missing
 * scroll listener, and the frozen clock leaves only the scroll handler. */
async function scrollFollowing(browser) {
  console.log("\n# A chat that scrolls with follow off: the pill stays on the frozen word");
  const server = createServer();
  const { page, errors } = await openFixture(browser, server, { follow: false });
  const target = 6;
  server.state.freeze = target * WORD_MS + 20;
  await startReading(page);
  for (let wait = 0; wait < 40; wait += 1) {
    if (await readMark(page)) break;
    await sleep(100);
  }
  const placed = await readMark(page);
  if (
    !check(placed !== null, "a pill exists under document.documentElement while the chat scrolls")
  ) {
    console.log("     page errors: " + JSON.stringify([...new Set(errors)]));
    await page.close();
    return;
  }
  check(settledOn(placed, (await readWordRects(page))[target]), "the pill starts on the word");

  const chatScroll = await page.evaluate(() => {
    const box = document.getElementById("scrollbox");
    box.scrollTop = 100;
    return { top: box.scrollTop, scrollable: box.scrollHeight > box.clientHeight };
  });
  check(chatScroll.scrollable, "the chat scrollport can scroll");
  check(chatScroll.top > 50, "the chat scrollport scrolled (" + chatScroll.top + " px)");
  await sleep(150);
  const afterChat = await readMark(page);
  check(
    afterChat !== null && settledOn(afterChat, (await readWordRects(page))[target]),
    "the pill follows the chat scroll"
  );

  await page.evaluate(() => window.scrollTo(0, 120));
  await sleep(150);
  const afterWindow = await readMark(page);
  check(
    afterWindow !== null && settledOn(afterWindow, (await readWordRects(page))[target]),
    "the pill follows the window scroll"
  );
  check(errors.length === 0, "no page error during the chat scroll");
  server.state.resolveGate();
  await page.close();
}

async function main() {
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    await movingReading(browser);
    await scrolledReading(browser);
    await scrollFollowing(browser);
  } finally {
    await browser.close();
  }
  console.log("");
  if (failures.length > 0) {
    console.log("caption pill checks FAILED (" + failures.length + ")");
    process.exit(1);
  }
  console.log("caption pill checks passed");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
