/**
 * Listening State Store
 *
 * Manages the global listening state for double-copy detection.
 * Shared between quick-settings.svelte and app-footer.svelte.
 */

import { isTauri } from "$lib/services/tauri.js";

let invoke: typeof import("@tauri-apps/api/core").invoke | null = null;

class ListeningStore {
    private _isListening = $state(true);
    private _error = $state<string | null>(null);
    private _isLoading = $state(false);

    get isListening(): boolean {
        return this._isListening;
    }

    get error(): string | null {
        return this._error;
    }

    get isLoading(): boolean {
        return this._isLoading;
    }

    async toggle(): Promise<void> {
        if (!isTauri || !invoke) {
            // In non-Tauri environment, just toggle locally
            this._isListening = !this._isListening;
            return;
        }

        try {
            const newState = !this._isListening;
            await invoke("set_listening", { enabled: newState });
            this._isListening = newState;
            this._error = null;
        } catch (e) {
            this._error = `${e}`;
            console.error("Failed to toggle listening state:", e);
        }
    }

    async setListening(enabled: boolean): Promise<void> {
        if (!isTauri || !invoke) {
            this._isListening = enabled;
            return;
        }

        try {
            await invoke("set_listening", { enabled });
            this._isListening = enabled;
            this._error = null;
        } catch (e) {
            this._error = `${e}`;
            console.error("Failed to set listening state:", e);
        }
    }

    async loadFromBackend(): Promise<void> {
        if (!isTauri || !invoke) return;

        try {
            const state = await invoke<boolean>("get_listening");
            this._isListening = state;
        } catch (e) {
            console.error("Failed to load listening state:", e);
        }
    }
}

export const listeningStore = new ListeningStore();

// Initialize on load
if (isTauri) {
    import("@tauri-apps/api/core")
        .then((core) => {
            invoke = core.invoke;
            // Load initial state from backend
            listeningStore.loadFromBackend();
        })
        .catch(() => { });
}
