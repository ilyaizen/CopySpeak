import { describe, expect, it } from "vite-plus/test";
import { COPYSPEECH_RYBBIT_SITE_ID, RYBBIT_HOST, rybbitScriptSrc } from "../web-analytics";

describe("rybbit script src", () => {
  it("loads the canonical /api/script.js from the analytics host", () => {
    const url = new URL(rybbitScriptSrc(COPYSPEECH_RYBBIT_SITE_ID));
    expect(url.origin).toBe(RYBBIT_HOST);
    expect(url.pathname).toBe("/api/script.js");
  });

  it("passes the site id in the query string", () => {
    const url = new URL(rybbitScriptSrc("8f8151b005e4"));
    expect(url.searchParams.get("siteId")).toBe("8f8151b005e4");
  });

  it("derives the tracker API base from the src, as the tracker does", () => {
    const src = rybbitScriptSrc(COPYSPEECH_RYBBIT_SITE_ID);
    expect(src.split("/script.js")[0]).toBe(`${RYBBIT_HOST}/api`);
  });
});
