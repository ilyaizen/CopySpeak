import { describe, it, expect } from "vitest";
import { render } from "@testing-library/svelte";

import ElevenLabsEngine from "./elevenlabs-engine.svelte";

function mockConfig() {
  return {
    tts: {
      elevenlabs: {
        api_key: "xi-test",
        voice_id: "21m00Tcm4TlvDq8ikWAM",
        voice_name: "Rachel",
        model_id: "eleven_turbo_v2_5",
        output_format: "mp3_44100_128",
        voice_stability: 0.5,
        voice_similarity_boost: 0.75,
        voice_style: 0,
        use_speaker_boost: false,
        use_manual_voice_id: false
      }
    }
  } as any;
}

describe("ElevenLabsEngine", () => {
  it("keeps model and format settings on the engine page", () => {
    render(ElevenLabsEngine, { localConfig: mockConfig() });

    expect(document.getElementById("model")).toBeTruthy();
    expect(document.getElementById("format")).toBeTruthy();
  });

  it("does not edit ElevenLabs voice or test the active profile engine", () => {
    const { container } = render(ElevenLabsEngine, { localConfig: mockConfig() });

    expect(document.getElementById("voice-mode")).toBeNull();
    expect(document.getElementById("voice-id")).toBeNull();
    expect(document.getElementById("voice-select")).toBeNull();
    expect(container.textContent).not.toContain("Test Engine");
  });
});
