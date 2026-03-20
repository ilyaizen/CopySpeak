/**
 * History item as returned from the Rust backend
 */
interface BackendHistoryEntry {
  id: string;
  timestamp: string;
  text: string;
  text_length: number;
  tts_engine: string;
  voice: string;
  speed: number;
  output_format?: string;
  output_path?: string;
  duration_ms?: number;
  batch_id?: string;
  app_name?: string;
  source?: string;
  filters_applied?: string[];
  success: boolean;
  error_message?: string;
  attempts: number;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

/**
 * Convert backend entry to frontend HistoryItem
 */
function backendToHistoryItem(entry: BackendHistoryEntry): HistoryItem {
  return {
    id: entry.id,
    timestamp: new Date(entry.timestamp).getTime(),
    text: entry.text,
    text_length: entry.text_length,
    tts_engine: entry.tts_engine as HistoryItem["tts_engine"],
    voice: entry.voice,
    speed: entry.speed,
    output_format: entry.output_format as HistoryItem["output_format"],
    output_path: entry.output_path,
    duration_ms: entry.duration_ms,
    batch_id: entry.batch_id,
    app_name: entry.app_name,
    source: entry.source,
    filters_applied: entry.filters_applied,
    success: entry.success,
    error_message: entry.error_message,
    attempts: entry.attempts,
    tags: entry.tags,
    metadata: entry.metadata,
  };
}

/**
 * Shared history state store for managing TTS history across the application
 * Uses Svelte 5 runes for reactive state management
 */

import type { HistoryItem, HistoryState, HistoryFilters, HistorySortOptions } from "$lib/types";
import {
  createEmptyHistoryState,
  filterHistoryItems,
  sortHistoryItems,
  calculateHistoryStatistics,
} from "$lib/models/history";

/**
 * Creates a history store with reactive state management
 */
function createHistoryStore() {
  // Core state
  let state = $state<HistoryState>(createEmptyHistoryState());

  // Derived filtered and sorted items
  let filters = $state<HistoryFilters>({});
  let sortOptions = $state<HistorySortOptions>({
    sort_by: "timestamp",
    order: "descending",
  });

  // Tauri invoke function (lazy loaded)
  let invoke: typeof import("@tauri-apps/api/core").invoke | null = null;

  /**
   * Initialize Tauri invoke function
   */
  async function initInvoke() {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return false;
    }

    if (!invoke) {
      try {
        const core = await import("@tauri-apps/api/core");
        invoke = core.invoke;
        return true;
      } catch {
        invoke = null;
        return false;
      }
    }
    return true;
  }

  /**
   * Load history from backend
   */
  async function loadHistory() {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      state.items = [];
      state.events = [];
      state.is_loading = false;
      state.error = "Tauri backend not available";
      return;
    }

    state.is_loading = true;
    state.error = null;

    try {
      const entries = await invoke<BackendHistoryEntry[]>("get_history");

      state.items = entries.map(backendToHistoryItem);

      state.statistics = calculateHistoryStatistics(state.items);
      state.last_updated = Date.now();
    } catch (e) {
      state.error = `Failed to load history: ${e}`;
      state.items = [];
    } finally {
      state.is_loading = false;
    }
  }

  /**
   * Refresh history data
   */
  async function refresh() {
    await loadHistory();
  }

  /**
   * Add a new history item (optimistic update)
   */
  function addItem(item: HistoryItem) {
    state.items = [item, ...state.items];
    state.statistics = calculateHistoryStatistics(state.items);
    state.last_updated = Date.now();
  }

  /**
   * Update an existing history item
   */
  function updateItem(id: string, updates: Partial<HistoryItem>) {
    const index = state.items.findIndex((item) => item.id === id);
    if (index !== -1) {
      state.items[index] = { ...state.items[index], ...updates };
      state.statistics = calculateHistoryStatistics(state.items);
      state.last_updated = Date.now();
    }
  }

  /**
   * Delete a history item
   */
  async function deleteItem(id: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("delete_history_entry", { entryId: id });
      state.items = state.items.filter((item) => item.id !== id);
      state.statistics = calculateHistoryStatistics(state.items);
      state.last_updated = Date.now();
    } catch (e) {
      throw new Error(`Failed to delete history entry: ${e}`);
    }
  }

  /**
   * Delete multiple history items
   */
  async function deleteItems(ids: string[]) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    const errors: string[] = [];
    for (const id of ids) {
      try {
        await invoke("delete_history_entry", { entryId: id });
      } catch (e) {
        errors.push(`${id}: ${e}`);
      }
    }

    if (errors.length > 0) {
      throw new Error(`Failed to delete some entries: ${errors.join(", ")}`);
    }

    state.items = state.items.filter((item) => !ids.includes(item.id));
    state.statistics = calculateHistoryStatistics(state.items);
    state.last_updated = Date.now();
  }

  /**
   * Clear all history
   */
  async function clearAll() {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("clear_history");
      state.items = [];
      state.events = [];
      state.statistics = calculateHistoryStatistics([]);
      state.last_updated = Date.now();
    } catch (e) {
      throw new Error(`Failed to clear history: ${e}`);
    }
  }

  /**
   * Set filter criteria
   */
  function setFilters(newFilters: HistoryFilters) {
    filters = newFilters;
  }

  /**
   * Update filter criteria (merge with existing)
   */
  function updateFilters(partialFilters: Partial<HistoryFilters>) {
    filters = { ...filters, ...partialFilters };
  }

  /**
   * Clear all filters
   */
  function clearFilters() {
    filters = {};
  }

  /**
   * Set sort options
   */
  function setSortOptions(newSortOptions: HistorySortOptions) {
    sortOptions = newSortOptions;
  }

  /**
   * Get filtered items (derived state)
   */
  function getFilteredItems(): HistoryItem[] {
    let items = state.items;

    // Apply filters
    if (Object.keys(filters).length > 0) {
      items = filterHistoryItems(items, filters);
    }

    // Apply sorting
    items = sortHistoryItems(items, sortOptions);

    return items;
  }

  /**
   * Get item count after filters
   */
  function getFilteredCount(): number {
    return getFilteredItems().length;
  }

  /**
   * Play a history entry
   */
  async function playEntry(id: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      const item = state.items.find((i) => i.id === id);
      if (item) {
        await invoke("show_hud_for_playback", { 
          text: item.text, 
          audioDurationMs: item.duration_ms ?? null 
        }).catch(() => {});
      }
      await invoke("play_history_entry", { entryId: id });
    } catch (e) {
      throw new Error(`Failed to play entry: ${e}`);
    }
  }

  /**
   * Re-speak a history entry
   */
  async function reSpeakEntry(id: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("speak_history_entry", { entryId: id });
    } catch (e) {
      throw new Error(`Failed to re-speak entry: ${e}`);
    }
  }

  /**
   * Copy entry text to clipboard
   */
  async function copyEntryText(id: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("copy_history_entry_text", { entryId: id });
    } catch (e) {
      throw new Error(`Failed to copy entry text: ${e}`);
    }
  }

  /**
   * Get all entries in a batch
   */
  async function getBatch(batchId: string): Promise<HistoryItem[]> {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      const entries = await invoke<BackendHistoryEntry[]>("get_history_batch", { batchId });
      return entries.map(backendToHistoryItem);
    } catch (e) {
      throw new Error(`Failed to get batch: ${e}`);
    }
  }

  /**
   * Play all entries in a batch sequentially
   */
  async function playBatch(batchId: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("play_history_batch", { batchId });
    } catch (e) {
      throw new Error(`Failed to play batch: ${e}`);
    }
  }

  /**
   * Delete all entries in a batch
   */
  async function deleteBatch(batchId: string) {
    const canInvoke = await initInvoke();
    if (!canInvoke || !invoke) {
      throw new Error("Tauri backend not available");
    }

    try {
      await invoke("delete_history_batch", { batchId });
      state.items = state.items.filter((item) => item.batch_id !== batchId);
      state.statistics = calculateHistoryStatistics(state.items);
      state.last_updated = Date.now();
    } catch (e) {
      throw new Error(`Failed to delete batch: ${e}`);
    }
  }

  return {
    // Reactive state accessors
    get items() {
      return state.items;
    },
    get events() {
      return state.events;
    },
    get statistics() {
      return state.statistics;
    },
    get config() {
      return state.config;
    },
    get isLoading() {
      return state.is_loading;
    },
    get error() {
      return state.error;
    },
    get lastUpdated() {
      return state.last_updated;
    },
    get filters() {
      return filters;
    },
    get sortOptions() {
      return sortOptions;
    },

    // Computed/derived values
    getFilteredItems,
    getFilteredCount,

    // Actions
    loadHistory,
    refresh,
    addItem,
    updateItem,
    deleteItem,
    deleteItems,
    clearAll,
    setFilters,
    updateFilters,
    clearFilters,
    setSortOptions,
    playEntry,
    reSpeakEntry,
    copyEntryText,
    getBatch,
    playBatch,
    deleteBatch,
  };
}

// Export singleton store instance
export const historyStore = createHistoryStore();
