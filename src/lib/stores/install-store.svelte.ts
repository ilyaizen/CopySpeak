/**
 * Install Store
 *
 * Owns the single persistent `install-progress` listener and per-engine run
 * state (log lines, per-voice status, running flag, exit code). Lives for the
 * app lifetime so an install keeps streaming even while the dialog is
 * dismissed; the dialog and the engine-setup chip are pure views over this.
 */

import { isTauri } from "$lib/services/tauri.js";

export type VoiceStatus = "pending" | "installing" | "done" | "failed";

export interface InstallRun {
  lines: string[];
  voiceStatus: Record<string, VoiceStatus>;
  running: boolean;
  exitCode: number | null;
}

function newRun(voices: string[]): InstallRun {
  const voiceStatus: Record<string, VoiceStatus> = {};
  for (const v of voices) voiceStatus[v] = "pending";
  return { lines: [], voiceStatus, running: true, exitCode: null };
}

class InstallStore {
  private _runs = $state<Record<string, InstallRun>>({});
  private unlisten: (() => void) | null = null;
  private started = false;

  get runs(): Readonly<Record<string, InstallRun>> {
    return this._runs;
  }

  run(engine: string): InstallRun | undefined {
    return this._runs[engine];
  }

  isRunning(engine: string): boolean {
    return this._runs[engine]?.running ?? false;
  }

  /** Begin a run: reset state and mark the given voices pending. */
  async start(engine: string, voices: string[]) {
    this._runs[engine] = newRun(voices);
    await this.init();
  }

  /** Re-queue a single failed voice without disturbing the rest of a run. */
  markVoice(engine: string, id: string, status: VoiceStatus) {
    const run = this._runs[engine];
    if (run) run.voiceStatus[id] = status;
  }

  // Parse a `[STEP]/[DONE]/[ERROR] voice:<id>` marker into a status update.
  private applyMarker(run: InstallRun, line: string) {
    const m = line.match(/^\s*\[(STEP|DONE|ERROR)\]\s*voice:(\S+)/);
    if (!m) return;
    const status: VoiceStatus =
      m[1] === "STEP" ? "installing" : m[1] === "DONE" ? "done" : "failed";
    run.voiceStatus[m[2]] = status;
  }

  // Flip every still-in-flight voice to its terminal status (done on exit 0,
  // failed otherwise). Applies uniformly: shared engines never emit per-voice
  // markers, so their voices land here; per-voice engines settle earlier.
  private finalize(run: InstallRun, ok: boolean) {
    for (const id of Object.keys(run.voiceStatus)) {
      const s = run.voiceStatus[id];
      if (s === "pending" || s === "installing") run.voiceStatus[id] = ok ? "done" : "failed";
    }
  }

  async init() {
    if (this.started || !isTauri) return;
    this.started = true;
    try {
      const { listen } = await import("@tauri-apps/api/event");
      this.unlisten = await listen<{
        engine: string;
        line: string | null;
        done: boolean;
        exit_code: number | null;
      }>("install-progress", (e) => {
        const p = e.payload;
        let run = this._runs[p.engine];
        if (!run) {
          // Late arrival (listener started mid-run): track without resetting.
          run = newRun([]);
          this._runs[p.engine] = run;
        }
        if (p.line) {
          run.lines.push(p.line);
          this.applyMarker(run, p.line);
        }
        if (p.done) {
          run.running = false;
          run.exitCode = p.exit_code;
          this.finalize(run, p.exit_code === 0);
        }
      });
    } catch (e) {
      console.error("Failed to listen for install-progress:", e);
      this.started = false;
    }
  }

  /** Detach the progress listener. Unused in production (the store lives for the
   *  app lifetime) but exposed for tests/teardown so the handle is read. */
  destroy() {
    this.unlisten?.();
    this.unlisten = null;
    this.started = false;
  }
}

export const installStore = new InstallStore();
