import { test, expect } from "@playwright/test";
import path from "node:path";
test("real DOM acceptance, native interval, controls, mutation and cleanup (transport fixture)", async ({
  page,
  browser
}, info) => {
  console.log(`${info.project.name}: ${browser.version()}`);
  await page.route("http://localhost/fixture", (r) =>
    r.fulfill({
      contentType: "text/html; charset=utf-8",
      // The page colors its text with oklch like modern pages do: Chromium
      // serializes the probe's computed currentColor space-preserving as
      // oklch(...), so this exercises the oklch parse through the real
      // engine's computed style, not a test mock.
      body: '<main><p id="source" style="color:oklch(0.2 0.01 190)">same <b>same</b> 😀 שלום</p><p id="other">other</p></main>'
    })
  );
  await page.goto("http://localhost/fixture");
  await page.evaluate(() => {
    // SAFETY: test fixture pokes an untyped chrome stub onto the browser page; window typing cannot know it.
    const w = window as any;
    w.sent = [];
    w.settings = { hoverRead: true, showPanel: true };
    w.chrome = {
      storage: {
        local: {
          get: async () => w.settings,
          set: async (value: any) => {
            Object.assign(w.settings, value);
            w.settingsChanged({}, "local");
          }
        },
        onChanged: { addListener: (f: any) => (w.settingsChanged = f) }
      },
      runtime: {
        // The hover button resolves its icon through runtime.getURL at load.
        getURL: (asset: string) => `chrome-extension://copyspeak/${asset}`,
        sendMessage: async (m: any) => {
          w.sent.push(m);
          if (m.type === "site-access") return true;
        },
        onMessage: { addListener: (f: any) => (w.receive = f) }
      }
    };
    const r = document.createRange();
    r.selectNodeContents(document.querySelector("#source")!);
    getSelection()!.addRange(r);
  });
  await page.addScriptTag({ path: path.resolve(import.meta.dirname, "../dist/content.js") });
  const capture = await page.evaluate(() => {
    let result: any;
    // SAFETY: receive is the chrome stub installed above; page context is untyped by design.
    (window as any).receive({ type: "capture", request_id: "q" }, {}, (x: any) => (result = x));
    return result;
  });
  expect(capture.text).toBe("same same 😀 שלום");
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(capture.text);
  await page.evaluate(() => {
    // SAFETY: receive/token are chrome stubs installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: (window as any).token,
        event: { v: 1, type: "accepted", request_id: "q", reading_id: "r" }
      },
      {},
      () => {}
    );
  });
  // Document token returned by capture, not guessed by the native host.
  await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: { v: 1, type: "accepted", request_id: "q", reading_id: "r" }
      },
      {},
      () => {}
    );
  }, capture.document_token);
  // The capture clears the native selection: no faded highlight stays on the page.
  expect(await page.evaluate(() => getSelection()!.rangeCount)).toBe(0);
  // Reading paints only the pill; the page keeps its own background and styles.
  expect(await page.evaluate(() => [...CSS.highlights.keys()])).toEqual([]);
  expect(await page.evaluate(() => document.querySelectorAll(".copyspeak-mark").length)).toBe(0);
  expect(
    await page.evaluate(() =>
      document.querySelector<HTMLElement>("#source")?.style.getPropertyValue("isolation")
    )
  ).toBe("");
  await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: {
          v: 1,
          type: "state",
          reading_id: "r",
          seq: 1,
          status: "playing",
          word: { start: 5, end: 9 },
          word_available: true
        }
      },
      {},
      () => {}
    );
  }, capture.document_token);
  const firstPill = await page.evaluate(() => {
    const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
    if (mark === null) throw new Error("the pill is not painted");
    return {
      left: mark.style.left,
      transform: mark.style.transform,
      blend: mark.style.mixBlendMode,
      fill: getComputedStyle(mark).backgroundColor
    };
  });
  expect(firstPill.blend).toBe("difference");
  // The white fill under difference is the bubble: it inverts the word.
  expect(firstPill.fill).toBe("rgb(255, 255, 255)");
  // The first word settles instantly: there is no previous position to glide from.
  expect(firstPill.transform).toBe("");
  // The animated caption: a second word glides the pill, then settles on it.
  // The tween starts synchronously before its first rendered frame: the pill
  // carries the offset from the previous word in `transform`.
  const glideStart = await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: {
          v: 1,
          type: "state",
          reading_id: "r",
          seq: 2,
          status: "playing",
          word: { start: 10, end: 12 },
          word_available: true
        }
      },
      {},
      () => {}
    );
    const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
    return mark === null ? null : { left: mark.style.left, transform: mark.style.transform };
  }, capture.document_token);
  expect(glideStart).not.toBeNull();
  expect(glideStart!.transform).not.toBe("");
  expect(glideStart!.left).not.toBe(firstPill.left);
  // The pill settles over the second word once the glide ends.
  await expect
    .poll(() =>
      page.evaluate(() => {
        const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
        // The missing pill reads as a distinct value, never as a settled pill.
        return mark === null ? null : { left: mark.style.left, transform: mark.style.transform };
      })
    )
    .toEqual({ left: glideStart!.left, transform: "" });
  // A wordless mid-reading frame (caption gap, fragment tail) holds the
  // paint: the word and pill stay, so the next word still glides from them.
  await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: {
          v: 1,
          type: "state",
          reading_id: "r",
          seq: 3,
          status: "playing",
          word: null,
          word_available: true
        }
      },
      {},
      () => {}
    );
  }, capture.document_token);
  expect(
    await page.evaluate(() => {
      const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
      return mark === null ? null : { left: mark.style.left, transform: mark.style.transform };
    })
  ).toEqual({ left: glideStart!.left, transform: "" });
  // Turning the animation off in settings jumps the pill instead.
  await page.evaluate(() => {
    // SAFETY: chrome.storage is installed by this test's browser fixture.
    return (window as any).chrome.storage.local.set({ captionAnimation: false });
  });
  const jumped = await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed earlier in this test; page context is untyped.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: {
          v: 1,
          type: "state",
          reading_id: "r",
          seq: 4,
          status: "playing",
          word: { start: 13, end: 17 },
          word_available: true
        }
      },
      {},
      () => {}
    );
    const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
    return mark === null ? null : { left: mark.style.left, transform: mark.style.transform };
  }, capture.document_token);
  expect(jumped).not.toBeNull();
  expect(jumped!.transform).toBe("");
  expect(jumped!.left).not.toBe(glideStart!.left);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  expect(
    await page.evaluate(() => {
      // SAFETY: sent is the chrome stub installed earlier in this test; page context is untyped.
      return (window as any).sent.at(-1).action;
    })
  ).toBe("pause");
  await page.getByRole("button", { name: "Hide playback box", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeHidden();
  expect(await page.evaluate(() => document.querySelectorAll(".copyspeak-mark").length)).toBe(1);
  expect(
    await page.evaluate(() => {
      // SAFETY: sent is installed by this test's browser fixture.
      return (window as any).sent.at(-1).action;
    })
  ).toBe("pause");
  await page.evaluate(() => {
    // SAFETY: chrome.storage is installed by this test's browser fixture.
    return (window as any).chrome.storage.local.set({ showPanel: true });
  });
  await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeVisible();
  await page.locator("#source b").evaluate((el) => (el.textContent = "changed"));
  await expect
    .poll(() => page.evaluate(() => document.querySelectorAll(".copyspeak-mark").length))
    .toBe(0);
  expect(
    await page.evaluate(() => {
      // SAFETY: sent is the chrome stub installed earlier in this test; page context is untyped.
      return (window as any).sent.at(-1).action;
    })
  ).toBe("stop");
  // Hover reading reuses exact DOM offsets without replacing an existing selection.
  await page.locator("#other").evaluate((el) => {
    const range = document.createRange();
    range.selectNodeContents(el);
    getSelection()!.addRange(range);
  });
  await page.locator("#source").hover({ position: { x: 4, y: 4 } });
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeech" })).toBeVisible();
  await page.getByRole("button", { name: "Read paragraph with CopySpeech" }).click();
  expect(
    await page.evaluate(() => {
      // SAFETY: sent is installed by this test's browser fixture.
      return (window as any).sent.at(-1).type;
    })
  ).toBe("read-paragraph");
  const paragraph = await page.evaluate(() => {
    let reply: any;
    // SAFETY: receive is installed by this test's browser fixture.
    (window as any).receive(
      { type: "capture", request_id: "paragraph", paragraph: true },
      {},
      (value: any) => {
        reply = value;
      }
    );
    return reply;
  });
  expect(paragraph.text).toBe("same changed 😀 שלום");
  await page.evaluate((token) => {
    // SAFETY: receive is installed by this test's browser fixture.
    return (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: {
          v: 1,
          type: "accepted",
          request_id: "paragraph",
          reading_id: "paragraph-reading"
        }
      },
      {},
      () => {}
    );
  }, paragraph.document_token);
  expect(await page.evaluate(() => getSelection()!.toString())).toBe("other");
  await page.locator("#other").hover({ position: { x: 4, y: 4 } });
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeech" })).toBeVisible();
  await page.evaluate(() => {
    // SAFETY: chrome.storage is installed by this test's browser fixture.
    return (window as any).chrome.storage.local.set({ hoverRead: false });
  });
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeech" })).toHaveCount(0);
  expect(await page.evaluate(() => CSS.highlights.has("copyspeak-hover"))).toBe(false);
});

test("follows the spoken word into view", async ({ page }) => {
  const lines = (prefix: string) =>
    Array.from({ length: 40 }, (_, i) => `<p>${prefix} ${i} pad pad pad pad pad</p>`).join("");
  await page.route("http://localhost/follow", (r) =>
    r.fulfill({
      contentType: "text/html; charset=utf-8",
      // The target word sits below the fold with plenty of page after it, so
      // the follow scroll can place it on the anchor line.
      body: `<main>${lines("line")}<p id="last">targetword</p>${lines("after")}</main>`
    })
  );
  await page.goto("http://localhost/follow");
  await page.evaluate(() => {
    // SAFETY: test fixture pokes an untyped chrome stub onto the page; window typing cannot know it.
    const w = window as any;
    w.sent = [];
    w.chrome = {
      storage: {
        local: { get: async () => ({}), set: async () => {} },
        onChanged: { addListener: () => {} }
      },
      runtime: {
        getURL: (asset: string) => `chrome-extension://copyspeak/${asset}`,
        sendMessage: async (m: any) => {
          w.sent.push(m);
          if (m.type === "site-access") return true;
        },
        onMessage: { addListener: (f: any) => (w.receive = f) }
      }
    };
    const range = document.createRange();
    range.selectNodeContents(document.querySelector("main")!);
    getSelection()!.addRange(range);
  });
  await page.addScriptTag({ path: path.resolve(import.meta.dirname, "../dist/content.js") });
  const capture = await page.evaluate(() => {
    let result: any;
    // SAFETY: receive is the chrome stub installed above; page context is untyped by design.
    (window as any).receive(
      { type: "capture", request_id: "follow" },
      {},
      (x: any) => (result = x)
    );
    return result;
  });
  await page.evaluate((token) => {
    // SAFETY: receive is the chrome stub installed above; page context is untyped by design.
    (window as any).receive(
      {
        type: "native",
        document_token: token,
        event: { v: 1, type: "accepted", request_id: "follow", reading_id: "f" }
      },
      {},
      () => {}
    );
  }, capture.document_token);
  const start = capture.text.indexOf("targetword");
  expect(start).toBeGreaterThan(-1);
  await page.evaluate(
    ({ token, start }) => {
      // SAFETY: receive is the chrome stub installed above; page context is untyped by design.
      (window as any).receive(
        {
          type: "native",
          document_token: token,
          event: {
            v: 1,
            type: "state",
            reading_id: "f",
            seq: 1,
            status: "playing",
            word: { start, end: start + "targetword".length },
            word_available: true
          }
        },
        {},
        () => {}
      );
    },
    { token: capture.document_token, start }
  );
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  // The smooth scroll ends with the pill on the anchor line, inside the viewport.
  const pillTop = () =>
    page.evaluate(() => {
      const mark = document.querySelector<HTMLElement>(".copyspeak-mark");
      // A missing pill reads as infinity, so the poll keeps waiting for it.
      return mark === null
        ? Number.POSITIVE_INFINITY
        : Math.round(mark.getBoundingClientRect().top);
    });
  await expect.poll(pillTop).toBeLessThan(500);
  expect(await pillTop()).toBeGreaterThan(0);
});
