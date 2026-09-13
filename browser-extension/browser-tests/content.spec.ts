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
      body: '<main><p id="source">same <b>same</b> 😀 שלום</p><p id="other">other</p></main>'
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
  await page.addScriptTag({ path: path.resolve("browser-extension/dist/content.js") });
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
  expect(await page.evaluate(() => getSelection()!.rangeCount)).toBe(0);
  expect(await page.evaluate(() => CSS.highlights.has("copyspeak-passage"))).toBe(true);
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
  expect(
    await page.evaluate(() => {
      // SAFETY: highlight ranges are Range/StaticRange set members; the cast only widens to Range which has toString.
      return [...CSS.highlights.get("copyspeak-word")!].map((r) => (r as Range).toString());
    })
  ).toEqual(["same"]);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  expect(
    await page.evaluate(() => {
      // SAFETY: sent is the chrome stub installed earlier in this test; page context is untyped.
      return (window as any).sent.at(-1).action;
    })
  ).toBe("pause");
  await page.getByRole("button", { name: "Hide playback box", exact: true }).click();
  await expect(page.getByRole("button", { name: "Pause", exact: true })).toBeHidden();
  expect(await page.evaluate(() => CSS.highlights.has("copyspeak-word"))).toBe(true);
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
  await expect.poll(() => page.evaluate(() => CSS.highlights.has("copyspeak-passage"))).toBe(false);
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
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeak" })).toBeVisible();
  await page.getByRole("button", { name: "Read paragraph with CopySpeak" }).click();
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
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeak" })).toBeVisible();
  await page.evaluate(() => {
    // SAFETY: chrome.storage is installed by this test's browser fixture.
    return (window as any).chrome.storage.local.set({ hoverRead: false });
  });
  await expect(page.getByRole("button", { name: "Read paragraph with CopySpeak" })).toHaveCount(0);
  expect(await page.evaluate(() => CSS.highlights.has("copyspeak-hover"))).toBe(false);
});
