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

/* The plugin's control-server calls, answered in the page: one list per route. */
window.__speaks = [];
const realFetch = window.fetch;
window.fetch = function (url, init) {
  const target = String(url);
  if (target.indexOf("43117") === -1) return realFetch(url, init);
  if (target.indexOf("/speak") !== -1) {
    window.__speaks.push(JSON.parse(init.body));
    return Promise.resolve(new Response("", { status: 200 }));
  }
  if (target.indexOf("/status") !== -1) {
    return Promise.resolve(
      new Response(JSON.stringify({ active: false }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
  }
  return Promise.resolve(new Response("", { status: 200 }));
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
`;

const html = `<!doctype html>
<html><head>
<script>${runtime}</${"script"}>
<script src="/client.js"></${"script"}>
<script>${driver}</${"script"}>
</head><body></body></html>`;

const HISTORY = [{ turn: 1, seq: 3, messageId: "m1", text: "First reply from history" }];

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
