import { describe, expect, test } from "vite-plus/test";
import {
  isHistoryEffective,
  isPostProcessEffective,
  isDuckEffective
} from "#lib/experimental-gates";

describe("isHistoryEffective", () => {
  test("returns true when both experimental and history are enabled", () => {
    expect(
      isHistoryEffective({
        general: { experimental_enabled: true },
        history: { enabled: true }
      })
    ).toBe(true);
  });

  test("returns false when experimental is off even if history is enabled", () => {
    expect(
      isHistoryEffective({
        general: { experimental_enabled: false },
        history: { enabled: true }
      })
    ).toBe(false);
  });

  test("returns false when experimental is enabled but history is off", () => {
    expect(
      isHistoryEffective({
        general: { experimental_enabled: true },
        history: { enabled: false }
      })
    ).toBe(false);
  });

  test("returns false for legacy config where experimental was undefined", () => {
    expect(
      isHistoryEffective({
        general: {},
        history: { enabled: true }
      })
    ).toBe(false);
  });

  test("handles null or undefined config safely", () => {
    expect(isHistoryEffective(null)).toBe(false);
    expect(isHistoryEffective(undefined)).toBe(false);
  });
});

describe("isPostProcessEffective", () => {
  test("returns true when both experimental and post_process are enabled", () => {
    expect(
      isPostProcessEffective({
        general: { experimental_enabled: true },
        post_process: { enabled: true }
      })
    ).toBe(true);
  });

  test("returns false when experimental is off", () => {
    expect(
      isPostProcessEffective({
        general: { experimental_enabled: false },
        post_process: { enabled: true }
      })
    ).toBe(false);
  });

  test("returns false when post_process is off", () => {
    expect(
      isPostProcessEffective({
        general: { experimental_enabled: true },
        post_process: { enabled: false }
      })
    ).toBe(false);
  });
});

describe("isDuckEffective", () => {
  test("returns true when both experimental and duck are enabled", () => {
    expect(
      isDuckEffective({
        general: { experimental_enabled: true },
        playback: { duck: { enabled: true } }
      })
    ).toBe(true);
  });

  test("returns false when experimental is off", () => {
    expect(
      isDuckEffective({
        general: { experimental_enabled: false },
        playback: { duck: { enabled: true } }
      })
    ).toBe(false);
  });

  test("returns false when duck is off", () => {
    expect(
      isDuckEffective({
        general: { experimental_enabled: true },
        playback: { duck: { enabled: false } }
      })
    ).toBe(false);
  });
});
