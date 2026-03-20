/**
 * History data model and access layer
 * Provides typed access to TTS history data and operations
 */

import type {
  HistoryEvent,
  HistoryItem,
  HistoryFilters,
  HistorySortOptions,
  HistoryPaginationOptions,
  HistoryQueryResult,
  HistoryStatistics,
  HistoryState,
  AudioFormat,
  TtsEngine,
} from "$lib/types";

/**
 * Creates a new history item with default values
 */
export function createHistoryItem(
  text: string,
  ttsEngine: TtsEngine,
  voice: string,
  speed: number,
  overrides: Partial<HistoryItem> = {}
): HistoryItem {
  const id = generateHistoryId();
  const timestamp = Date.now();

  return {
    id,
    timestamp,
    text,
    text_length: text.length,
    tts_engine: ttsEngine,
    voice,
    speed,
    success: false,
    attempts: 0,
    ...overrides,
  };
}

/**
 * Creates a new history event with default values
 */
export function createHistoryEvent(
  eventType: HistoryItem["id"],
  text: string,
  overrides: Partial<HistoryEvent> = {}
): HistoryEvent {
  const id = generateHistoryId();
  const timestamp = Date.now();

  return {
    id,
    timestamp,
    event_type: eventType as any,
    text,
    success: false,
    ...overrides,
  };
}

/**
 * Generates a unique history ID
 */
export function generateHistoryId(): string {
  return `hist_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Formats a timestamp to a readable date string
 */
export function formatHistoryDate(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleString();
}

/**
 * Formats a timestamp to ISO date string (YYYY-MM-DD)
 */
export function formatHistoryDateISO(timestamp: number): string {
  const date = new Date(timestamp);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * Gets the hour from a timestamp (0-23)
 */
export function getHourFromTimestamp(timestamp: number): number {
  const date = new Date(timestamp);
  return date.getHours();
}

/**
 * Filters history items based on filter criteria
 */
export function filterHistoryItems(
  items: HistoryItem[],
  filters: HistoryFilters
): HistoryItem[] {
  return items.filter((item) => {
    // Text search filter
    if (filters.search_text) {
      const searchLower = filters.search_text.toLowerCase();
      if (!item.text.toLowerCase().includes(searchLower)) {
        return false;
      }
    }

    // Engine filter
    if (filters.tts_engine && item.tts_engine !== filters.tts_engine) {
      return false;
    }

    // Voice filter
    if (filters.voice && item.voice !== filters.voice) {
      return false;
    }

    // Date range filter
    if (filters.date_from && item.timestamp < filters.date_from) {
      return false;
    }
    if (filters.date_to && item.timestamp > filters.date_to) {
      return false;
    }

    // Success filter
    if (filters.success_only && !item.success) {
      return false;
    }

    // Failed filter
    if (filters.failed_only && item.success) {
      return false;
    }

    // Tags filter
    if (filters.tags && filters.tags.length > 0) {
      if (!item.tags || !filters.tags.some((tag) => item.tags?.includes(tag))) {
        return false;
      }
    }

    // App name filter
    if (filters.app_name && item.app_name !== filters.app_name) {
      return false;
    }

    return true;
  });
}

/**
 * Sorts history items based on sort options
 */
export function sortHistoryItems(
  items: HistoryItem[],
  sortOptions: HistorySortOptions
): HistoryItem[] {
  const sorted = [...items];

  sorted.sort((a, b) => {
    let compareValue = 0;

    switch (sortOptions.sort_by) {
      case "timestamp":
        compareValue = a.timestamp - b.timestamp;
        break;
      case "text":
        compareValue = a.text.localeCompare(b.text);
        break;
      case "duration":
        compareValue = (a.duration_ms ?? 0) - (b.duration_ms ?? 0);
        break;
      case "engine":
        compareValue = a.tts_engine.localeCompare(b.tts_engine);
        break;
    }

    return sortOptions.order === "ascending"
      ? compareValue
      : -compareValue;
  });

  return sorted;
}

/**
 * Paginates history items
 */
export function paginateHistoryItems(
  items: HistoryItem[],
  pagination: HistoryPaginationOptions
): HistoryItem[] {
  const { limit, offset } = pagination;
  return items.slice(offset, offset + limit);
}

/**
 * Queries history items with filters, sorting, and pagination
 */
export function queryHistoryItems(
  items: HistoryItem[],
  filters?: HistoryFilters,
  sortOptions?: HistorySortOptions,
  pagination?: HistoryPaginationOptions
): HistoryQueryResult {
  let result = items;

  // Apply filters
  if (filters) {
    result = filterHistoryItems(result, filters);
  }

  const totalCount = result.length;

  // Apply sorting
  if (sortOptions) {
    result = sortHistoryItems(result, sortOptions);
  }

  // Apply pagination
  if (pagination) {
    const paginatedItems = paginateHistoryItems(result, pagination);
    return {
      items: paginatedItems,
      total_count: totalCount,
      limit: pagination.limit,
      offset: pagination.offset,
    };
  }

  return {
    items: result,
    total_count: totalCount,
    limit: result.length,
    offset: 0,
  };
}

/**
 * Calculates statistics from history items
 */
export function calculateHistoryStatistics(
  items: HistoryItem[]
): HistoryStatistics {
  if (items.length === 0) {
    return {
      total_items: 0,
      total_duration_ms: 0,
      successful_items: 0,
      failed_items: 0,
      success_rate: 0,
      by_engine: {} as Record<TtsEngine, number>,
      by_format: {} as Record<AudioFormat, number>,
      by_hour: {},
      by_day: {},
      most_used_voice: null,
      average_text_length: 0,
      average_duration_ms: 0,
    };
  }

  const stats: HistoryStatistics = {
    total_items: items.length,
    total_duration_ms: 0,
    successful_items: 0,
    failed_items: 0,
    success_rate: 0,
    by_engine: {} as Record<TtsEngine, number>,
    by_format: {} as Record<AudioFormat, number>,
    by_hour: {},
    by_day: {},
    most_used_voice: null,
    average_text_length: 0,
    average_duration_ms: 0,
  };

  const voiceCounts: Record<string, number> = {};
  let totalTextLength = 0;

  items.forEach((item) => {
    // Duration stats
    if (item.duration_ms) {
      stats.total_duration_ms += item.duration_ms;
    }

    // Success/failure stats
    if (item.success) {
      stats.successful_items++;
    } else {
      stats.failed_items++;
    }

    // Engine stats
    if (!stats.by_engine[item.tts_engine]) {
      stats.by_engine[item.tts_engine] = 0;
    }
    stats.by_engine[item.tts_engine]++;

    // Format stats
    if (item.output_format) {
      if (!stats.by_format[item.output_format]) {
        stats.by_format[item.output_format] = 0;
      }
      stats.by_format[item.output_format]++;
    }

    // Hour stats
    const hour = getHourFromTimestamp(item.timestamp);
    if (!stats.by_hour[hour]) {
      stats.by_hour[hour] = 0;
    }
    stats.by_hour[hour]++;

    // Day stats
    const day = formatHistoryDateISO(item.timestamp);
    if (!stats.by_day[day]) {
      stats.by_day[day] = 0;
    }
    stats.by_day[day]++;

    // Voice stats
    voiceCounts[item.voice] = (voiceCounts[item.voice] ?? 0) + 1;

    // Text length
    totalTextLength += item.text_length;
  });

  // Calculate average text length
  stats.average_text_length = totalTextLength / items.length;

  // Calculate average duration
  if (stats.total_duration_ms > 0) {
    stats.average_duration_ms = stats.total_duration_ms / items.length;
  }

  // Calculate success rate
  stats.success_rate =
    items.length > 0 ? stats.successful_items / items.length : 0;

  // Find most used voice
  let maxCount = 0;
  for (const [voice, count] of Object.entries(voiceCounts)) {
    if (count > maxCount) {
      maxCount = count;
      stats.most_used_voice = voice;
    }
  }

  return stats;
}

/**
 * Gets items from today
 */
export function getHistoryItemsFromToday(items: HistoryItem[]): HistoryItem[] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const todayTimestamp = today.getTime();

  const tomorrow = new Date(today);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const tomorrowTimestamp = tomorrow.getTime();

  return items.filter(
    (item) =>
      item.timestamp >= todayTimestamp && item.timestamp < tomorrowTimestamp
  );
}

/**
 * Gets items from the last N days
 */
export function getHistoryItemsFromLastDays(
  items: HistoryItem[],
  days: number
): HistoryItem[] {
  const now = Date.now();
  const daysInMs = days * 24 * 60 * 60 * 1000;
  const cutoffTime = now - daysInMs;

  return items.filter((item) => item.timestamp >= cutoffTime);
}

/**
 * Removes duplicate items (keeping the most recent)
 */
export function removeDuplicateHistoryItems(
  items: HistoryItem[]
): HistoryItem[] {
  const seen = new Map<string, HistoryItem>();

  // Process in reverse order to keep the most recent
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    const key = `${item.text}|${item.tts_engine}|${item.voice}`;

    if (!seen.has(key)) {
      seen.set(key, item);
    }
  }

  return Array.from(seen.values());
}

/**
 * Creates an empty history state
 */
export function createEmptyHistoryState(): HistoryState {
  return {
    items: [],
    events: [],
    statistics: calculateHistoryStatistics([]),
    config: {
      enabled: true,
      storage_mode: "temp",
      persistent_dir: null,
      auto_delete: { keep_latest: 50 },
      cleanup_orphaned_files: true,
    },
    is_loading: false,
    error: null,
    last_updated: 0,
  };
}

/**
 * Merges history states (e.g., from different sources)
 */
export function mergeHistoryStates(
  ...states: HistoryState[]
): HistoryState {
  if (states.length === 0) {
    return createEmptyHistoryState();
  }

  const mergedItems: HistoryItem[] = [];
  const mergedEvents: HistoryEvent[] = [];
  const seenIds = new Set<string>();

  states.forEach((state) => {
    state.items.forEach((item) => {
      if (!seenIds.has(item.id)) {
        mergedItems.push(item);
        seenIds.add(item.id);
      }
    });

    state.events.forEach((event) => {
      if (!seenIds.has(event.id)) {
        mergedEvents.push(event);
        seenIds.add(event.id);
      }
    });
  });

  return {
    items: mergedItems,
    events: mergedEvents,
    statistics: calculateHistoryStatistics(mergedItems),
    config: states[states.length - 1].config,
    is_loading: false,
    error: null,
    last_updated: Date.now(),
  };
}

// Re-export HTML export utilities for convenience
export {
  generateHistoryHtmlPage,
  generatePageHeader,
  generateTableOfContents,
  generateStatisticsSection,
  generateHistoryTableSection,
  generateHistoryRowHtml,
  generatePageFooter,
  generateCss,
  generateCompactHtmlReport,
  generateTimelineHtml,
  generateTimelineItemHtml,
  escapeHtml,
  type HtmlExportOptions,
} from "./html-export";
