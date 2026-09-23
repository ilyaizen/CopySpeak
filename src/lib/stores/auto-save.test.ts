import { describe, expect, it, beforeEach, afterEach, vi } from "vite-plus/test";
import { syncSaveBarOrAutoSave, flushAutoSave, AUTO_SAVE_DEBOUNCE_MS } from "./auto-save.svelte";
import { saveBar } from "./save-bar.svelte";

// The auto-save store registers callbacks and timers; these tests drive it
// with fake timers so debounce behavior is deterministic.
describe("syncSaveBarOrAutoSave", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    flushAutoSave();
    vi.useRealTimers();
  });

  function makeSave() {
    let save = 0;
    return {
      bump: () => {
        save += 1;
      },
      read: () => save,
      onSave: async () => {
        save += 1;
      },
      onCancel: () => {}
    };
  }

  it("shows the save bar when auto-save is off", () => {
    const h = makeSave();
    syncSaveBarOrAutoSave(true, false, h.onSave, h.onCancel, "Save", "Cancel");
    expect(saveBar.visible).toBe(true);
    vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS * 2);
    expect(h.read()).toBe(0);
  });

  it("hides the save bar and auto-saves after the debounce when on", () => {
    const h = makeSave();
    syncSaveBarOrAutoSave(true, true, h.onSave, h.onCancel, "Save", "Cancel");
    expect(saveBar.visible).toBe(false);
    expect(h.read()).toBe(0);
    vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS - 1);
    expect(h.read()).toBe(0);
    vi.advanceTimersByTime(1);
    expect(h.read()).toBe(1);
  });

  it("collapses a burst of edits into one save", () => {
    const h = makeSave();
    // Simulate 5 quick edits: each re-runs the effect while changes are pending.
    for (let i = 0; i < 5; i++) {
      syncSaveBarOrAutoSave(true, true, h.onSave, h.onCancel, "Save", "Cancel");
      vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS / 2);
    }
    expect(h.read()).toBe(0);
    vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS);
    expect(h.read()).toBe(1);
  });

  it("does not save when changes are reverted before the debounce fires", () => {
    const h = makeSave();
    syncSaveBarOrAutoSave(true, true, h.onSave, h.onCancel, "Save", "Cancel");
    // User reverts: effect re-runs with hasChanges=false.
    syncSaveBarOrAutoSave(false, true, h.onSave, h.onCancel, "Save", "Cancel");
    vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS * 2);
    expect(h.read()).toBe(0);
    expect(saveBar.visible).toBe(false);
  });

  it("flushAutoSave saves immediately instead of waiting", () => {
    const h = makeSave();
    syncSaveBarOrAutoSave(true, true, h.onSave, h.onCancel, "Save", "Cancel");
    flushAutoSave();
    expect(h.read()).toBe(1);
    // Idempotent: a second flush is a no-op.
    flushAutoSave();
    expect(h.read()).toBe(1);
    vi.advanceTimersByTime(AUTO_SAVE_DEBOUNCE_MS * 2);
    expect(h.read()).toBe(1);
  });
});
