import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";

import OpenAiEngine from "./openai-engine.svelte";

function mockConfig() {
  return {
    tts: {
      openai: { api_key: "sk-test", model: "tts-1", voice: "alloy" }
    }
  } as any;
}

describe("OpenAiEngine", () => {
  it("keeps model settings on the engine page", () => {
    render(OpenAiEngine, { localConfig: mockConfig() });

    expect(document.getElementById("openai-model")).toBeTruthy();
  });

  it("does not edit OpenAI voice or test the active profile engine", () => {
    const { container } = render(OpenAiEngine, { localConfig: mockConfig() });

    expect(document.getElementById("openai-voice")).toBeNull();
    expect(container.textContent).not.toContain("Test Engine");
  });
});
