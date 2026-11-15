/*
 * Auto-read gate fixture.
 *
 * Loads the real copyspeak-dsh/client.js unmodified into a stub-host page,
 * mounts the registered AutoReader component with a fake Chat store and a fake
 * Session store, and asserts which replies POST /speak.
 *
 * The bug this guards: Chat history loads AFTER the composer dock mounts, so
 * the last loaded reply looks like a new turn and reads aloud on every DSH
 * restart. Only a turn that runs while watched may read.
 *
 * The fixture also drives the caption reader: partial auto-read speaks the top
 * and bottom paragraphs, so its first fragment jumps over the omitted middle.
 * The highlighted word must still land on the spoken word.
 *
 * Run from the repo root:  bun scripts/test-dsh-plugin-auto-read.mjs
 */
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
/* AUTO_READ_CLIENT points the fixture at another build, such as the committed
   or the pre-fix copy, to prove the checks fail on the bug. */
const clientSrc = readFileSync(
  process.env.AUTO_READ_CLIENT ?? join(here, "..", "copyspeak-dsh", "client.js"),
  "utf8"
);
if (clientSrc.includes("</" + "script>")) throw new Error("client.js contains a script closer");

/* Minimal hooks runtime: AutoReader renders null, so it needs useRef, useEffect
   and a way to re-run the component when a fake store publishes. */
const runtime = `
window.__rt = (function () {
  let hooks = [];
  let cursor = 0;
  let effects = [];
  let component = null;
  let props = null;

  const React = {
    createElement: function (type, p, ...children) {
      return { type: type, props: p, children: children };
    },
    Fragment: Symbol("Fragment"),
    useRef: function (initial) {
      const index = cursor++;
      if (hooks[index] === undefined) hooks[index] = { current: initial };
      return hooks[index];
    },
    useEffect: function (fn, deps) {
      const index = cursor++;
      const previous = hooks[index];
      const changed =
        previous === undefined ||
        deps === undefined ||
        deps.length !== previous.deps.length ||
        deps.some(function (dep, at) {
          return !Object.is(dep, previous.deps[at]);
        });
      if (!changed) return;
      hooks[index] = { deps: deps };
      effects.push(fn);
    }
  };

  function run() {
    cursor = 0;
    effects = [];
    component(props);
    const pending = effects;
    effects = [];
    for (const fn of pending) fn();
  }

  return {
    React: React,
    mount: function (fn, nextProps) {
      component = fn;
      props = nextProps;
      hooks = [];
      run();
    },
    run: run
  };
})();

/* The plugin's control-server calls, answered in the page: one list per route.
   A caption scenario holds /speak open, because the app answers it only when
   playback ends; a held reading keeps the status poll running so the inline
   highlight can be observed. */
window.__speaks = [];
window.__status = null;
window.__holdSpeak = false;
window.__heldSpeaks = [];
const realFetch = window.fetch;
window.fetch = function (url, init) {
  const target = String(url);
  if (target.indexOf("43117") === -1) return realFetch(url, init);
  if (target.indexOf("/speak") !== -1) {
    window.__speaks.push(JSON.parse(init.body));
    if (window.__holdSpeak) {
      return new Promise(function (resolve) {
        window.__heldSpeaks.push(resolve);
      });
    }
    return Promise.resolve(new Response("", { status: 200 }));
  }
  if (target.indexOf("/status") !== -1) {
    const status = window.__status === null ? { active: false } : window.__status;
    return Promise.resolve(
      new Response(JSON.stringify(status), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
  }
  return Promise.resolve(new Response("", { status: 200 }));
};
/* Resolve a held /speak as the app does when the reading finishes. */
window.__releaseSpeaks = function () {
  const held = window.__heldSpeaks.splice(0);
  for (const resolve of held) resolve(new Response("", { status: 200 }));
};

window.__ModuleLoader__ = {
  load(def) {
    const mod = def.factory(function (name) {
      if (name === "react") return window.__rt.React;
      throw new Error("no stub for " + name);
    });
    mod.apply({
      effect: function (fn) {
        return fn();
      },
      locale: {
        register: function () {
          return function () {};
        },
        bind: function () {
          return function (key) {
            return key;
          };
        }
      },
      slots: {
        inject: function (key, callback) {
          callback();
          return function () {};
        },
        register: function (options, fn) {
          if (options.name === "conversation.composer.dock") window.__autoReader = fn;
          return function () {};
        }
      },
      sessions: {
        binding: function () {
          return undefined;
        }
      },
      uiConversation: {
        interactions: {
          register: function () {
            return function () {};
          }
        }
      }
    });
  }
};
`;

/* One fake Chat snapshot. A turn carries a turn-tail only once it closed, and
   its assistant-step node carries the same durable message id. Built inside the
   page because a snapshot holds functions Playwright cannot serialize. */
const driver = `
/* One fake Chat snapshot: a turn carries a turn-tail only once it closed, and
   its assistant-step node carries the same durable message id. */
function chatSnapshot(turns) {
  const order = [];
  const nodes = new Map();
  const turnsMap = new Map();
  for (const turn of turns) {
    const key = "assistant-" + turn.turn;
    const blocks = [{ kind: "text", text: turn.text }];
    order.push(key);
    nodes.set(key, {
      kind: "assistant-step",
      data: { finalNode: { seq: turn.seq, messageId: turn.messageId }, blocks: blocks }
    });
    const tail = {
      closing: { finalNode: { seq: turn.seq, messageId: turn.messageId }, blocks: blocks }
    };
    turnsMap.set(turn.turn, {
      turn: turn.turn,
      start: { seq: turn.seq - 2 },
      end: { seq: turn.seq },
      status: turn.open === true ? "open" : "closed",
      data: {
        get: function (kind) {
          return kind === "turn-tail" ? tail : undefined;
        }
      }
    });
  }
  if (turns.length === 0) {
    return {
      order: [],
      nodes: { get: function () { return undefined; }, values: function () { return []; } },
      timeline: { turnOrder: [], turns: new Map() }
    };
  }
  return {
    order: order,
    nodes: {
      get: function (key) {
        return nodes.get(key);
      },
      values: function () {
        return [...nodes.values()];
      }
    },
    timeline: { turnOrder: turns.map((turn) => turn.turn), turns: turnsMap }
  };
}

window.__auto = (function () {
  let chat = chatSnapshot([]);
  let session = { openState: "cold", running: false };

  function mount() {
    window.__rt.mount(window.__autoReader, {
      useChat: function (selector) {
        return selector(chat);
      },
      useSession: function (selector) {
        return selector(session);
      }
    });
  }

  return {
    /* A session switch mounts a fresh watcher: reset the stores first, then the
       hooks, exactly as the host does when another session opens. */
    reset: function (turns, nextSession) {
      chat = chatSnapshot(turns);
      session = nextSession;
      mount();
    },
    setChat: function (turns) {
      chat = chatSnapshot(turns);
      window.__rt.run();
    },
    setSession: function (next) {
      session = next;
      window.__rt.run();
    },
    speaks: function () {
      return window.__speaks.slice();
    }
  };
})();

/* The rendered reply the caption reader addresses. Chat marks each row with
   data-chat-flow-kind and the durable node key, and each settled Markdown body
   with data-text-interaction-source. */
window.__caption = (function () {
  function render(text, key) {
    const row = document.createElement("div");
    row.setAttribute("data-chat-flow-kind", "assistant-step");
    row.setAttribute("data-chat-node-key", key);
    const source = document.createElement("div");
    source.setAttribute("data-text-interaction-source", "");
    for (const block of text.split(/\\n\\s*\\n/)) {
      const paragraph = document.createElement("p");
      paragraph.textContent = block;
      source.appendChild(paragraph);
    }
    row.appendChild(source);
    document.body.appendChild(row);
    return row;
  }

  /* One caption fragment as /status reports it: word offsets index the spoken
     fragment text, and each word carries an audible window. */
  function status(text, fragmentIndex, positionMs) {
    const words = [];
    let index = 0;
    for (const match of text.matchAll(/\\S+/gu)) {
      const start = match.index;
      const end = start + match[0].length;
      words.push({
        text_start: start,
        text_end: end,
        start_ms: index * 1000,
        end_ms: index * 1000 + 1000
      });
      index += 1;
    }
    return {
      active: true,
      paused: false,
      captions: { text: text, words: words },
      position_ms: positionMs,
      duration_ms: words.length * 1000,
      fragment_index: fragmentIndex
    };
  }

  return { render: render, status: status };
})();
`;

const html = `<!doctype html>
<html><head>
<script>${runtime}</${"script"}>
<script src="/client.js"></${"script"}>
<script>${driver}</${"script"}>
</head><body></body></html>`;

const HISTORY = [{ turn: 1, seq: 3, messageId: "m1", text: "First reply from history" }];
const FIVE_PARAGRAPHS = ["P1", "P2", "P3", "P4", "P5"].join("\n\n");
/* Five long paragraphs: the omitted middle must be wider than the caption
   alignment window, or a contiguous walk would still reach the bottom
   excerpt. Every word is unique, so a match proves the alignment. */
const CAPTION_PARAGRAPHS = Array.from({ length: 5 }, function (_, paragraph) {
  return Array.from({ length: 20 }, function (_, word) {
    return "w" + (paragraph + 1) + "x" + (word + 1);
  }).join(" ");
}).join("\n\n");

async function settle(page) {
  await page.waitForTimeout(120);
}

let failures = 0;
function check(name, actual, expected) {
  const ok = JSON.stringify(actual) === JSON.stringify(expected);
  if (!ok) failures += 1;
  console.log(
    (ok ? "PASS " : "FAIL ") +
      name +
      (ok ? "" : " — expected " + JSON.stringify(expected) + ", got " + JSON.stringify(actual))
  );
}

async function scenario(page, label, steps) {
  await page.evaluate(() => {
    window.__speaks.length = 0;
    window.__status = null;
    window.__holdSpeak = false;
    window.__releaseSpeaks();
    for (const row of document.querySelectorAll('[data-chat-flow-kind="assistant-step"]')) {
      row.remove();
    }
  });
  await page.evaluate(() => window.__auto.reset([], { openState: "cold", running: false }));
  await settle(page);
  await steps(page);
  const speaks = await page.evaluate(() => window.__speaks.map((body) => body.text));
  console.log("[" + label + "] speaks: " + JSON.stringify(speaks));
  return speaks;
}

async function main() {
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage({ viewport: { width: 900, height: 600 } });
  page.on("pageerror", (error) => {
    failures += 1;
    console.log("[pageerror]", error.message);
  });
  page.on("console", (message) => {
    if (message.type() === "error") console.log("[console]", message.text());
  });

  await page.addInitScript(() => {
    localStorage.setItem("dsh.copyspeak-voice.auto-read", "1");
    localStorage.setItem("dsh.copyspeak-voice.captions", "1");
  });
  await page.route("**/fixture.local/**", async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === "/client.js") {
      return route.fulfill({ status: 200, contentType: "text/javascript", body: clientSrc });
    }
    return route.fulfill({ status: 200, contentType: "text/html", body: html });
  });

  await page.goto("http://fixture.local/", { waitUntil: "load" });
  await page.waitForFunction(() => window.__autoReader !== undefined, null, { timeout: 5000 });

  const setChat = (turns) => page.evaluate((next) => window.__auto.setChat(next), turns);
  const setSession = (next) => page.evaluate((state) => window.__auto.setSession(state), next);

  /* Restart with an open session: history lands while the session still loads. */
  let speaks = await scenario(page, "restart: history before open", async () => {
    await setChat(HISTORY);
    await settle(page);
    await setSession({ openState: "open", running: false });
    await settle(page);
  });
  check("restart: loaded history never reads", speaks, []);

  /* The host may deliver history in pieces: an empty window, then the reply. */
  speaks = await scenario(page, "restart: history after open", async () => {
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setChat(HISTORY);
    await settle(page);
  });
  check("restart: history arriving after open never reads", speaks, []);

  /* A new turn in an open session reads once, whichever publication lands first. */
  speaks = await scenario(page, "live turn: reply after running", async () => {
    await setChat(HISTORY);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setSession({ openState: "open", running: true });
    await settle(page);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setChat([
      ...HISTORY,
      { turn: 2, seq: 6, messageId: "m2", text: "Second reply from a live turn" }
    ]);
    await settle(page);
    await setChat([
      ...HISTORY,
      { turn: 2, seq: 6, messageId: "m2", text: "Second reply from a live turn" }
    ]);
    await settle(page);
  });
  check("live turn: one reading of the new reply", speaks, ["Second reply from a live turn"]);

  /* A queued turn keeps the session running; the reply that lands while the
     next turn runs must still read. */
  speaks = await scenario(page, "queued turns", async () => {
    await setChat(HISTORY);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setSession({ openState: "open", running: true });
    await settle(page);
    await setChat([...HISTORY, { turn: 2, seq: 6, messageId: "m2", text: "Reply of turn two" }]);
    await settle(page);
    await setChat([
      ...HISTORY,
      { turn: 2, seq: 6, messageId: "m2", text: "Reply of turn two" },
      { turn: 3, seq: 9, messageId: "m3", text: "Reply of turn three" }
    ]);
    await settle(page);
    await setSession({ openState: "open", running: false });
    await settle(page);
  });
  check("queued turns: both replies read", speaks, ["Reply of turn two", "Reply of turn three"]);

  /* A brand-new session has no history: its first reply must read. */
  speaks = await scenario(page, "fresh session", async () => {
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setSession({ openState: "open", running: true });
    await settle(page);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setChat([{ turn: 1, seq: 3, messageId: "n1", text: "First ever reply" }]);
    await settle(page);
  });
  check("fresh session: first reply reads", speaks, ["First ever reply"]);

  /* Partial auto-read: reload with the excerpt preferences, because the plugin
     reads localStorage once at load. */
  async function reloadWithExcerpt(excerpt) {
    await page.evaluate((next) => {
      localStorage.setItem("dsh.copyspeak-voice.excerpt", "1");
      if (next.mode === undefined) {
        localStorage.removeItem("dsh.copyspeak-voice.excerpt-mode");
      } else {
        localStorage.setItem("dsh.copyspeak-voice.excerpt-mode", next.mode);
      }
      if (next.top === undefined) {
        localStorage.removeItem("dsh.copyspeak-voice.excerpt-top");
      } else {
        localStorage.setItem("dsh.copyspeak-voice.excerpt-top", String(next.top));
      }
      if (next.bottom === undefined) {
        localStorage.removeItem("dsh.copyspeak-voice.excerpt-bottom");
      } else {
        localStorage.setItem("dsh.copyspeak-voice.excerpt-bottom", String(next.bottom));
      }
    }, excerpt);
    await page.reload({ waitUntil: "load" });
    await page.waitForFunction(() => window.__autoReader !== undefined, null, { timeout: 5000 });
  }

  async function liveReply(page, text) {
    await setChat(HISTORY);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setSession({ openState: "open", running: true });
    await settle(page);
    await setSession({ openState: "open", running: false });
    await settle(page);
    await setChat([...HISTORY, { turn: 2, seq: 6, messageId: "m2", text }]);
    await settle(page);
  }

  speaks = await scenario(page, "excerpt: off", async () => {
    await liveReply(page, FIVE_PARAGRAPHS);
  });
  check("excerpt off reads the whole reply", speaks, [FIVE_PARAGRAPHS]);

  await reloadWithExcerpt({});
  speaks = await scenario(page, "excerpt: top and bottom", async () => {
    await liveReply(page, FIVE_PARAGRAPHS);
  });
  check("excerpt: keeps two top and two bottom paragraphs", speaks, [
    "P1\n\nP2\n\nP4\n\nP5"
  ]);

  speaks = await scenario(page, "excerpt: short reply", async () => {
    await liveReply(page, "A\n\nB\n\nC");
  });
  check("excerpt: short reply stays whole", speaks, ["A\n\nB\n\nC"]);

  await reloadWithExcerpt({ mode: "top", top: 1 });
  speaks = await scenario(page, "excerpt: top only", async () => {
    await liveReply(page, FIVE_PARAGRAPHS);
  });
  check("excerpt: top only keeps the first paragraph", speaks, ["P1"]);

  await reloadWithExcerpt({ mode: "bottom", bottom: 1 });
  speaks = await scenario(page, "excerpt: bottom only", async () => {
    await liveReply(page, FIVE_PARAGRAPHS);
  });
  check("excerpt: bottom only keeps the last paragraph", speaks, ["P5"]);

  /* The caption reader aligns the spoken fragment onto the rendered reply.
     The app strips newlines before it speaks, so an excerpt fragment is one
     flat run of words: the top block is followed directly by the bottom block,
     with the omitted middle absent from the caption text. */
  async function paintReport(page) {
    return page.evaluate(() => {
      const highlight =
        window.CSS !== undefined && window.CSS.highlights !== undefined
          ? window.CSS.highlights.get("copyspeak-read-word")
          : undefined;
      const ranges = [];
      if (highlight !== undefined) {
        for (const range of highlight) ranges.push(range.toString());
      }
      return {
        marks: document.querySelectorAll(".dsh-csva-mark").length,
        text: ranges.join(" ")
      };
    });
  }

  async function captionPaints(label, replyText) {
    await scenario(page, label, async () => {
      await page.evaluate((value) => {
        window.__holdSpeak = true;
        window.__caption.render(value, "assistant-2");
      }, replyText);
      await liveReply(page, replyText);
      await page.waitForFunction(() => window.__speaks.length >= 1, null, { timeout: 3000 });
      const spoken = await page.evaluate(() => window.__speaks[0].text);
      /* The app sanitizes the fragment before speaking: newlines become
         spaces, so the caption text is one flat run. */
      await page.evaluate((text) => {
        window.__status = window.__caption.status(text, 0, 50);
      }, spoken.replace(/\s+/g, " "));
      await page.waitForTimeout(400);
      const paint = await paintReport(page);
      console.log("[" + label + "] paint: " + JSON.stringify(paint));
      check(label + ": paints the spoken word", paint.text, "w1x1");
      await page.evaluate(() => {
        window.__status = null;
        window.__holdSpeak = false;
        window.__releaseSpeaks();
      });
    });
  }

  await page.evaluate(() => localStorage.removeItem("dsh.copyspeak-voice.excerpt"));
  await page.reload({ waitUntil: "load" });
  await page.waitForFunction(() => window.__autoReader !== undefined, null, { timeout: 5000 });
  await captionPaints("caption: whole reply aligns", CAPTION_PARAGRAPHS);

  await reloadWithExcerpt({ mode: "both", top: 1, bottom: 2 });
  await captionPaints("caption: excerpt jump aligns", CAPTION_PARAGRAPHS);

  /* The app paginates a long excerpt at sentence boundaries, so the first
     fragment can carry the top paragraph and the start of the bottom one.
     The second fragment then continues inside the bottom paragraph. */
  const captionBlocks = CAPTION_PARAGRAPHS.split("\n\n").map((block) => block.split(" "));
  const firstFragment = [captionBlocks[0].join(" "), captionBlocks[3].slice(0, 5).join(" ")].join(
    " "
  );
  const secondFragment = [
    captionBlocks[3].slice(5).join(" "),
    captionBlocks[4].join(" ")
  ].join(" ");

  await scenario(page, "caption: excerpt fragments", async () => {
    await page.evaluate((value) => {
      window.__holdSpeak = true;
      window.__caption.render(value, "assistant-2");
    }, CAPTION_PARAGRAPHS);
    await liveReply(page, CAPTION_PARAGRAPHS);
    await page.waitForFunction(() => window.__speaks.length >= 1, null, { timeout: 3000 });
    await page.evaluate((text) => {
      window.__status = window.__caption.status(text, 0, 50);
    }, firstFragment);
    await page.waitForTimeout(400);
    const first = await paintReport(page);
    console.log("[caption: excerpt fragments] first paint: " + JSON.stringify(first));
    check("caption: first excerpt fragment paints", first.text, "w1x1");

    await page.evaluate((text) => {
      window.__status = window.__caption.status(text, 1, 50);
    }, secondFragment);
    await page.waitForTimeout(400);
    const second = await paintReport(page);
    console.log("[caption: excerpt fragments] second paint: " + JSON.stringify(second));
    check("caption: second excerpt fragment paints", second.text, "w4x6");
    await page.evaluate(() => {
      window.__status = null;
      window.__holdSpeak = false;
      window.__releaseSpeaks();
    });
  });

  await browser.close();
  if (failures > 0) {
    console.log(failures + " failure(s)");
    process.exit(1);
  }
  console.log("all auto-read checks passed");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
