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
    const w = window as any;
    w.sent = [];
    w.chrome = {
      runtime: {
        sendMessage: async (m: unknown) => {
          w.sent.push(m);
        },
        onMessage: { addListener: (f: unknown) => (w.receive = f) }
      }
    };
    const r = document.createRange();
    r.selectNodeContents(document.querySelector("#source")!);
    getSelection()!.addRange(r);
  });
  await page.addScriptTag({ path: path.resolve("browser-extension/dist/content.js") });
  const capture = await page.evaluate(() => {
    let result: any;
    (window as any).receive({ type: "capture", request_id: "q" }, {}, (x: any) => (result = x));
    return result;
  });
  expect(capture.text).toBe("same same 😀 שלום");
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(capture.text);
  await page.evaluate(() => {
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
    await page.evaluate(() => [...CSS.highlights.get("copyspeak-word")!].map((r) => r.toString()))
  ).toEqual(["same"]);
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  expect(await page.evaluate(() => (window as any).sent.at(-1).action)).toBe("pause");
  await page.locator("#source b").evaluate((el) => (el.textContent = "changed"));
  await expect.poll(() => page.evaluate(() => CSS.highlights.has("copyspeak-passage"))).toBe(false);
  expect(await page.evaluate(() => (window as any).sent.at(-1).action)).toBe("stop");
});
