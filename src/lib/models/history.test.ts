import { expect, it } from "vitest";
import {
  createHistoryItem,
  groupHistoryReadings,
  historyVoiceLabel,
  historyTimeAgo
} from "./history";
import type { EngineCatalogEntry, VoiceProfile } from "$lib/types";

it("shows elapsed time at minute, hour, and day boundaries", () => {
  const now = 200_000_000;
  const formatter = new Intl.RelativeTimeFormat(undefined, { style: "short" });
  expect(historyTimeAgo(now + 60_000, now)).toBe("Just now");
  expect(historyTimeAgo(now - 59_999, now)).toBe("Just now");
  expect(historyTimeAgo(now - 60_000, now)).toBe(formatter.format(-1, "minute"));
  expect(historyTimeAgo(now - 3_600_000, now)).toBe(formatter.format(-1, "hour"));
  expect(historyTimeAgo(now - 86_400_000, now)).toBe(formatter.format(-1, "day"));
});

it("uses matching voice labels without confusing engine IDs or exposing unknown IDs", () => {
  const item = createHistoryItem("Hello", "cartesia", "voice-id", 1);
  const profile: VoiceProfile = {
    id: "profile",
    name: "Reading",
    description: null,
    engine: "cartesia",
    voice: "voice-id",
    voice_label: "Katie",
    speed: 1,
    pitch: 1,
    effects: { enabled: false, active_effect: "none" },
    engine_options: {}
  };
  const engine: EngineCatalogEntry = {
    engine: "cartesia",
    label: "Cartesia",
    description: "",
    docs_url: "",
    supports_voice_refresh: false,
    supports_pitch: false,
    supports_bracket_emotes: false,
    options: [],
    voices: [
      {
        id: "voice-id",
        label: "Catalog Katie",
        language: null,
        description: null,
        gender: null,
        preview_url: null
      }
    ]
  };
  expect(historyVoiceLabel(item, [profile], [engine])).toBe("Katie");
  expect(historyVoiceLabel(item, [], [engine])).toBe("Catalog Katie");
  expect(historyVoiceLabel(item, [{ ...profile, engine: "openai" }], [])).toBe("Saved voice");
  expect(historyVoiceLabel(item, [], [])).toBe("Saved voice");
  expect(item.voice).toBe("voice-id");
});

it("presents fragments as one ordered reading without mutating source history", () => {
  const first = createHistoryItem("First", "openai", "alloy", 1, {
    id: "first",
    timestamp: 10,
    batch_id: "reading",
    success: true,
    output_path: "first.wav",
    duration_ms: 1000,
    metadata: { fragment_index: 0, fragment_total: 3 }
  });
  const last = createHistoryItem("Last", "openai", "alloy", 1, {
    id: "last",
    timestamp: 30,
    batch_id: "reading",
    success: false,
    metadata: { fragment_index: 1, fragment_total: 3 }
  });
  const single = createHistoryItem("Single", "openai", "alloy", 1, {
    id: "reading",
    timestamp: 20,
    success: true,
    output_path: "single.wav"
  });
  const items = [last, single, first];
  const readings = groupHistoryReadings(items);
  expect(readings.map((reading) => reading.text)).toEqual(["Single", "First\n\nLast"]);
  expect(readings[1]).toMatchObject({
    batchId: "reading",
    partCount: 3,
    timestamp: 10,
    durationMs: 1000,
    success: false,
    hasAudio: false
  });
  expect(readings[0].hasAudio).toBe(true);
  expect(readings[0].partCount).toBe(1);
  expect(items.map((item) => item.id)).toEqual(["last", "reading", "first"]);
  expect(groupHistoryReadings([])).toEqual([]);
});
