import { describe, expect, it } from "vite-plus/test";
import { firstEnvKeyConflict } from "./env-keys";

describe("firstEnvKeyConflict", () => {
  it("warns when a different key is typed over an env-sourced engine", () => {
    expect(
      firstEnvKeyConflict("xai", "XAI_API_KEY", { api_key: "typed-key" }, { api_key: "" })
    ).toEqual({
      engine: "xai",
      envVar: "XAI_API_KEY"
    });
  });

  it("stays silent when the value is unchanged", () => {
    const original = { api_key: "persisted-key" };
    expect(
      firstEnvKeyConflict("xai", "XAI_API_KEY", { api_key: "persisted-key" }, original)
    ).toBeNull();
  });

  it("stays silent when the typed key is empty or whitespace", () => {
    expect(
      firstEnvKeyConflict("xai", "XAI_API_KEY", { api_key: "   " }, { api_key: "" })
    ).toBeNull();
    expect(firstEnvKeyConflict("xai", "XAI_API_KEY", undefined, { api_key: "" })).toBeNull();
  });

  it("treats whitespace-only differences as unchanged", () => {
    expect(
      firstEnvKeyConflict("fish", "FISH_API_KEY", { api_key: "key1 " }, { api_key: "key1" })
    ).toBeNull();
  });
});
