/**
 * Structural predicates for features gated behind the Experimental master switch.
 * When general.experimental_enabled is false/missing, gated features are inactive
 * even if their individual switch was saved true.
 */

export interface HistoryGateConfig {
  general?: { experimental_enabled?: boolean };
  history?: { enabled?: boolean };
}

export interface PostProcessGateConfig {
  general?: { experimental_enabled?: boolean };
  post_process?: { enabled?: boolean };
}

export interface DuckGateConfig {
  general?: { experimental_enabled?: boolean };
  playback?: { duck?: { enabled?: boolean } };
}

export interface HudGateConfig {
  general?: { experimental_enabled?: boolean };
  hud?: { enabled?: boolean };
}

/** Resolves true only when both Experimental and History are enabled. */
export function isHistoryEffective(config?: HistoryGateConfig | null): boolean {
  return config?.general?.experimental_enabled === true && config?.history?.enabled === true;
}

/** Resolves true only when both Experimental and Post-process are enabled. */
export function isPostProcessEffective(config?: PostProcessGateConfig | null): boolean {
  return config?.general?.experimental_enabled === true && config?.post_process?.enabled === true;
}

/** Resolves true only when both Experimental and Duck are enabled. */
export function isDuckEffective(config?: DuckGateConfig | null): boolean {
  return config?.general?.experimental_enabled === true && config?.playback?.duck?.enabled === true;
}

/** Resolves true only when both Experimental and the HUD overlay are enabled. */
export function isHudEffective(config?: HudGateConfig | null): boolean {
  return config?.general?.experimental_enabled === true && config?.hud?.enabled === true;
}
