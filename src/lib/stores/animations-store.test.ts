import { describe, expect, it, vi } from "vite-plus/test";
import { testConfig } from "#lib/test-fixtures/app-config";

// The store reads matchMedia once at module import, so the stub must be in
// place before the dynamic import below resolves.
let matches = true;
const listeners = new Set<(event: { matches: boolean }) => void>();
vi.stubGlobal("matchMedia", () => ({
  matches,
  addEventListener: (_type: string, cb: (event: { matches: boolean }) => void) => {
    listeners.add(cb);
  },
  removeEventListener: (_type: string, cb: (event: { matches: boolean }) => void) => {
    listeners.delete(cb);
  }
}));

const { animationsStore } = await import("./animations-store.svelte");

function fireReducedMotion(next: boolean) {
  matches = next;
  for (const cb of listeners) cb({ matches: next });
}

describe("animations store", () => {
  it("treats a reduced-motion system request as an override of the saved preference", () => {
    // Import-time stub had matches = true: the system requests reduced motion.
    expect(animationsStore.reducedMotion).toBe(true);
    const config = testConfig();
    config.general.animations_enabled = true;
    animationsStore.syncFromConfig(config);
    expect(animationsStore.enabled).toBe(true);
    expect(animationsStore.effective).toBe(false);
  });

  it("re-enables animations when the system stops requesting reduced motion", () => {
    const config = testConfig();
    config.general.animations_enabled = true;
    animationsStore.syncFromConfig(config);
    expect(animationsStore.effective).toBe(false);

    fireReducedMotion(false);
    expect(animationsStore.reducedMotion).toBe(false);
    expect(animationsStore.effective).toBe(true);
  });

  it("keeps animations off when the saved preference is off", () => {
    fireReducedMotion(false);
    const config = testConfig();
    config.general.animations_enabled = false;
    animationsStore.syncFromConfig(config);
    expect(animationsStore.reducedMotion).toBe(false);
    expect(animationsStore.effective).toBe(false);
  });
});
