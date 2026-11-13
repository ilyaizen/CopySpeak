/**
 * Animations Preference Store
 *
 * Gates UI fade animations (route transitions, engines panel swaps) on the
 * saved general.animations_enabled preference. Self-syncs on config-changed,
 * like listening-store.
 */

import { isTauri } from "#lib/services/tauri.js";
import type { AppConfig } from "#lib/types";

let invoke: typeof import("@tauri-apps/api/core").invoke | null = null;
let listenFn: typeof import("@tauri-apps/api/event").listen | null = null;

class AnimationsStore {
  private _enabled = $state(true);
  private _reducedMotion = $state(false);

  get enabled(): boolean {
    return this._enabled;
  }

  /** The OS or browser asks for less motion; it overrides the saved preference. */
  get reducedMotion(): boolean {
    return this._reducedMotion;
  }

  /** Saved preference AND no reduced-motion request. Drives every fade and Lenis. */
  get effective(): boolean {
    return this._enabled && !this._reducedMotion;
  }

  syncFromConfig(config: AppConfig): void {
    this._enabled = config.general.animations_enabled;
  }

  async loadFromBackend(): Promise<void> {
    if (!isTauri || !invoke) return;

    try {
      const config = await invoke<AppConfig>("get_config");
      this.syncFromConfig(config);
    } catch (e) {
      console.error("Failed to load animations preference:", e);
    }
  }

  /** Track the OS/browser reduced-motion preference for the app's lifetime. */
  watchReducedMotion(): void {
    try {
      const motion = matchMedia("(prefers-reduced-motion: reduce)");
      this._reducedMotion = motion.matches;
      motion.addEventListener("change", (e) => {
        this._reducedMotion = e.matches;
      });
    } catch {
      // No matchMedia in this environment; reduced motion stays off.
    }
  }
}

export const animationsStore = new AnimationsStore();
animationsStore.watchReducedMotion();

if (isTauri) {
  Promise.all([
    import("@tauri-apps/api/core").then((core) => {
      invoke = core.invoke;
    }),
    import("@tauri-apps/api/event").then((event) => {
      listenFn = event.listen;
    })
  ])
    .then(() => {
      void animationsStore.loadFromBackend();
      if (listenFn) {
        void listenFn("config-changed", () => {
          void animationsStore.loadFromBackend();
        });
      }
    })
    .catch(() => {});
}
