/**
 * Barrel export for all model utilities
 * Provides convenient imports for history and HTML export functions
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
  mergeHistoryStates,

  // HTML export utilities
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
  escapeHtml
} from "./history";

// Type exports
export type { HtmlExportOptions, TemplateContext, ThemeColors } from "./html-export";

// Template backend exports
export { LIGHT_THEME, DARK_THEME, createTemplateContext, renderHtmlTemplate } from "./html-export";
