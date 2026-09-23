import { hideSaveBar, showSaveBar } from "./save-bar.svelte";

export const AUTO_SAVE_DEBOUNCE_MS = 800;

let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
let pendingSave: (() => Promise<void>) | null = null;

function clearTimer() {
  if (autoSaveTimer !== null) {
    clearTimeout(autoSaveTimer);
    autoSaveTimer = null;
  }
}

/**
 * Wire a page's change effect to either the manual Save/Cancel bar or
 * debounced auto-save, depending on the auto-save setting.
 *
 * Call this from a `$effect` that tracks the page's config state (i.e. reads
 * `hasChanges`); pass the SAME save/cancel handlers the save bar would get.
 * A burst of edits collapses into one save: the timer resets on every effect
 * run while `hasChanges` is true, and fires once after 800ms of quiet.
 *
 * When auto-save is off this is a passthrough to showSaveBar/hideSaveBar.
 */
export function syncSaveBarOrAutoSave(
  hasChanges: boolean,
  autoSaveEnabled: boolean,
  onSave: () => Promise<void>,
  onCancel: () => void,
  saveLabel: string,
  cancelLabel: string
) {
  if (!hasChanges) {
    clearTimer();
    pendingSave = null;
    hideSaveBar();
    return;
  }

  if (!autoSaveEnabled) {
    clearTimer();
    pendingSave = null;
    showSaveBar(onSave, onCancel, saveLabel, cancelLabel);
    return;
  }

  hideSaveBar();
  clearTimer();
  pendingSave = onSave;
  autoSaveTimer = setTimeout(() => {
    autoSaveTimer = null;
    const save = pendingSave;
    pendingSave = null;
    void save?.();
  }, AUTO_SAVE_DEBOUNCE_MS);
}

/** Flush any pending auto-save immediately (e.g. on component destroy). */
export function flushAutoSave() {
  if (autoSaveTimer === null) return;
  clearTimer();
  const save = pendingSave;
  pendingSave = null;
  void save?.();
}
