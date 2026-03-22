import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Service to handle Tauri API interactions.
 * Centralizes invoke calls for better error handling and debugging.
 */
export class TauriService {
  private static instance: TauriService;

  private constructor() {}

  public static getInstance(): TauriService {
    if (!TauriService.instance) {
      TauriService.instance = new TauriService();
    }
    return TauriService.instance;
  }

  /**
   * Wrapper for Tauri's invoke function with error handling and logging.
   * safely handles instances where Tauri API might not be available (e.g. browser dev)
   */
  public async invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    if (isTauri) {
      try {
        console.debug(`[TauriService] Invoking: ${cmd}`, args);
        const result = await tauriInvoke<T>(cmd, args);
        console.debug(`[TauriService] Result for ${cmd}:`, result);
        return result;
      } catch (error) {
        console.error(`[TauriService] Error invoking ${cmd}:`, error);
        throw error;
      }
    } else {
      console.warn(
        `[TauriService] Tauri API not available (browser mode). Mocking response for: ${cmd}`
      );
      // In a real app, you might want to return mock data here or throw a specific error
      // For now, we'll throw to mimic the behavior of a failed call, or return null if safe
      throw new Error(`Tauri API not available. Cannot invoke command: ${cmd}`);
    }
  }
}

export const tauriService = TauriService.getInstance();

// Export a direct invoke function for convenience, maintaining the signature
export const invoke = <T>(cmd: string, args?: Record<string, unknown>) =>
  tauriService.invoke<T>(cmd, args);
