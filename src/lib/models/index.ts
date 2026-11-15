/**
 * Barrel export for all model utilities
 * Provides convenient imports for history functions
 */

// History model exports
export {
  // Creation functions
  createHistoryItem,
  createHistoryEvent,
  generateHistoryId,

  // Formatting functions
  formatHistoryDate,
  formatHistoryDateISO,
  getHourFromTimestamp,

  // Filtering and sorting
  filterHistoryItems,
  sortHistoryItems,
  paginateHistoryItems,
  queryHistoryItems,

  // Statistics and analytics
  calculateHistoryStatistics,
  getHistoryItemsFromToday,
  getHistoryItemsFromLastDays,
  removeDuplicateHistoryItems,

  // State management
  createEmptyHistoryState,
  mergeHistoryStates
} from "./history";
