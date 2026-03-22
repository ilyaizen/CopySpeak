/**
 * HTML export template generation utilities
 * Provides functions to generate HTML representations of history data
 * suitable for viewing, printing, and sharing
 *
 * This module now uses a template-based rendering backend for cleaner
 * separation of concerns and better maintainability.
 */

import type { HistoryItem, HistoryStatistics } from "$lib/types";
import { formatHistoryDate, formatHistoryDateISO } from "./history";
import {
  createTemplateContext,
  renderHtmlTemplate,
  LIGHT_THEME as LIGHT_THEME_COLORS,
  DARK_THEME as DARK_THEME_COLORS
} from "./html-templates";

/**
 * Generates a complete HTML page for history export using template rendering
 */
export function generateHistoryHtmlPage(
  items: HistoryItem[],
  statistics?: HistoryStatistics,
  options: HtmlExportOptions = {}
): string {
  const {
    title = "CopySpeak History Export",
    includeStatistics = true,
    includeToc = items.length > 10,
    cssTheme = "light",
    dateRange
  } = options;

  // Create template context
  const context = createTemplateContext(items, {
    title,
    statistics,
    includeStatistics,
    includeToc,
    theme: cssTheme,
    dateRange
  });

  // Render using template backend
  return renderHtmlTemplate(context);
}

/**
 * Generates HTML page header with title and metadata
 */
export function generatePageHeader(
  title: string,
  dateRange?: { from: number; to: number }
): string {
  const exportDate = new Date().toLocaleString();
  const dateRangeText = dateRange
    ? ` (${formatHistoryDateISO(dateRange.from)} to ${formatHistoryDateISO(dateRange.to)})`
    : "";

  return `
    <header class="page-header">
      <h1>${escapeHtml(title)}</h1>
      <div class="metadata">
        <p>Exported: <span class="timestamp">${escapeHtml(exportDate)}</span></p>
        ${dateRangeText ? `<p>Date Range: <span class="date-range">${escapeHtml(dateRangeText)}</span></p>` : ""}
      </div>
    </header>`;
}

/**
 * Generates table of contents from history items
 */
export function generateTableOfContents(items: HistoryItem[]): string {
  const toc = `
    <nav class="table-of-contents">
      <h2>Table of Contents</h2>
      <ul>
        <li><a href="#statistics">Statistics</a></li>
        <li><a href="#history-items">History Items</a>
          <ul>
            <li>Total Items: ${items.length}</li>
            <li><a href="#by-engine">By Engine</a></li>
            <li><a href="#by-date">By Date</a></li>
          </ul>
        </li>
      </ul>
    </nav>`;

  return toc;
}

/**
 * Generates statistics section with charts and summary
 */
export function generateStatisticsSection(statistics: HistoryStatistics): string {
  const successPercentage = (statistics.success_rate * 100).toFixed(1);
  const totalDurationMinutes = (statistics.total_duration_ms / 1000 / 60).toFixed(1);

  return `
    <section id="statistics" class="statistics-section">
      <h2>Statistics</h2>

      <div class="stats-grid">
        <div class="stat-card">
          <h3>Total Items</h3>
          <p class="stat-value">${statistics.total_items}</p>
        </div>

        <div class="stat-card">
          <h3>Success Rate</h3>
          <p class="stat-value">${successPercentage}%</p>
          <p class="stat-detail">${statistics.successful_items} successful, ${statistics.failed_items} failed</p>
        </div>

        <div class="stat-card">
          <h3>Total Duration</h3>
          <p class="stat-value">${totalDurationMinutes} min</p>
          <p class="stat-detail">Avg: ${(statistics.average_duration_ms / 1000).toFixed(1)}s per item</p>
        </div>

        <div class="stat-card">
          <h3>Average Text Length</h3>
          <p class="stat-value">${statistics.average_text_length.toFixed(0)}</p>
          <p class="stat-detail">characters</p>
        </div>
      </div>

      <div class="stats-breakdown">
        <div class="breakdown-section">
          <h3>By TTS Engine</h3>
          <table class="breakdown-table">
            <thead>
              <tr>
                <th>Engine</th>
                <th>Count</th>
              </tr>
            </thead>
            <tbody>
              ${Object.entries(statistics.by_engine)
                .map(
                  ([engine, count]) => `<tr><td>${escapeHtml(engine)}</td><td>${count}</td></tr>`
                )
                .join("")}
            </tbody>
          </table>
        </div>

        <div class="breakdown-section">
          <h3>By Format</h3>
          <table class="breakdown-table">
            <thead>
              <tr>
                <th>Format</th>
                <th>Count</th>
              </tr>
            </thead>
            <tbody>
              ${Object.entries(statistics.by_format)
                .map(
                  ([format, count]) => `<tr><td>${escapeHtml(format)}</td><td>${count}</td></tr>`
                )
                .join("")}
            </tbody>
          </table>
        </div>

        ${
          statistics.most_used_voice
            ? `
        <div class="breakdown-section">
          <h3>Most Used Voice</h3>
          <p class="stat-detail">${escapeHtml(statistics.most_used_voice)}</p>
        </div>
        `
            : ""
        }
      </div>
    </section>`;
}

/**
 * Generates history items table with all details
 */
export function generateHistoryTableSection(items: HistoryItem[]): string {
  if (items.length === 0) {
    return `
      <section id="history-items" class="history-section">
        <h2>History Items</h2>
        <p class="empty-message">No items to display.</p>
      </section>`;
  }

  return `
    <section id="history-items" class="history-section">
      <h2>History Items (${items.length} total)</h2>
      <div class="table-wrapper">
        <table class="history-table">
          <thead>
            <tr>
              <th>Date & Time</th>
              <th>Text</th>
              <th>Length</th>
              <th>Engine</th>
              <th>Voice</th>
              <th>Speed</th>
              <th>Duration</th>
              <th>Format</th>
              <th>Status</th>
              <th>Tags</th>
            </tr>
          </thead>
          <tbody>
            ${items.map((item) => generateHistoryRowHtml(item)).join("")}
          </tbody>
        </table>
      </div>
    </section>`;
}

/**
 * Generates a single table row for a history item
 */
export function generateHistoryRowHtml(item: HistoryItem): string {
  const dateTime = formatHistoryDate(item.timestamp);
  const duration = item.duration_ms ? `${(item.duration_ms / 1000).toFixed(1)}s` : "—";
  const format = item.output_format || "—";
  const status = item.success
    ? '<span class="status-success">✓ Success</span>'
    : '<span class="status-error">✗ Failed</span>';
  const tags = item.tags
    ? item.tags.map((tag) => `<span class="tag">${escapeHtml(tag)}</span>`).join(" ")
    : "—";
  const textPreview =
    item.text.length > 100
      ? escapeHtml(item.text.substring(0, 100)) + "..."
      : escapeHtml(item.text);

  return `
    <tr class="history-row ${item.success ? "success" : "error"}">
      <td class="date-time">${escapeHtml(dateTime)}</td>
      <td class="text-cell" title="${escapeHtml(item.text)}">${textPreview}</td>
      <td class="text-length">${item.text_length}</td>
      <td class="engine">${escapeHtml(item.tts_engine)}</td>
      <td class="voice">${escapeHtml(item.voice)}</td>
      <td class="speed">${item.speed.toFixed(2)}x</td>
      <td class="duration">${duration}</td>
      <td class="format">${escapeHtml(format)}</td>
      <td class="status">${status}</td>
      <td class="tags">${tags}</td>
    </tr>`;
}

/**
 * Generates page footer with export information
 */
export function generatePageFooter(): string {
  const timestamp = new Date().toISOString();
  return `
    <footer class="page-footer">
      <p>Generated by CopySpeak - ${escapeHtml(timestamp)}</p>
    </footer>`;
}

/**
 * Generates CSS styles for the HTML export
 */
export function generateCss(theme: "light" | "dark" = "light"): string {
  const colors = theme === "light" ? LIGHT_THEME : DARK_THEME;

  return `
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }

    html, body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
      background-color: ${colors.background};
      color: ${colors.text};
      line-height: 1.6;
    }

    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 20px;
    }

    /* Header Styles */
    .page-header {
      border-bottom: 3px solid ${colors.accent};
      padding-bottom: 20px;
      margin-bottom: 30px;
    }

    .page-header h1 {
      font-size: 2.5em;
      color: ${colors.accent};
      margin-bottom: 10px;
    }

    .metadata {
      color: ${colors.muted};
      font-size: 0.95em;
    }

    .metadata p {
      margin: 5px 0;
    }

    /* Table of Contents */
    .table-of-contents {
      background-color: ${colors.cardBg};
      border-left: 4px solid ${colors.accent};
      padding: 15px;
      margin-bottom: 30px;
      border-radius: 4px;
    }

    .table-of-contents h2 {
      font-size: 1.3em;
      color: ${colors.accent};
      margin-bottom: 10px;
    }

    .table-of-contents ul {
      list-style: none;
      padding-left: 15px;
    }

    .table-of-contents li {
      margin: 5px 0;
    }

    .table-of-contents a {
      color: ${colors.link};
      text-decoration: none;
    }

    .table-of-contents a:hover {
      text-decoration: underline;
    }

    /* Statistics Section */
    .statistics-section {
      margin-bottom: 40px;
    }

    .statistics-section h2 {
      font-size: 1.8em;
      color: ${colors.accent};
      margin-bottom: 20px;
      padding-bottom: 10px;
      border-bottom: 2px solid ${colors.accent};
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 20px;
      margin-bottom: 30px;
    }

    .stat-card {
      background-color: ${colors.cardBg};
      border: 1px solid ${colors.border};
      border-radius: 8px;
      padding: 20px;
      text-align: center;
    }

    .stat-card h3 {
      font-size: 0.95em;
      color: ${colors.muted};
      text-transform: uppercase;
      margin-bottom: 10px;
      letter-spacing: 0.5px;
    }

    .stat-value {
      font-size: 2.5em;
      font-weight: bold;
      color: ${colors.accent};
      margin: 10px 0;
    }

    .stat-detail {
      font-size: 0.9em;
      color: ${colors.muted};
      margin-top: 8px;
    }

    /* Statistics Breakdown */
    .stats-breakdown {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 20px;
      margin-top: 20px;
    }

    .breakdown-section {
      background-color: ${colors.cardBg};
      border: 1px solid ${colors.border};
      border-radius: 8px;
      padding: 20px;
    }

    .breakdown-section h3 {
      color: ${colors.accent};
      margin-bottom: 15px;
      font-size: 1.1em;
    }

    .breakdown-table {
      width: 100%;
      border-collapse: collapse;
    }

    .breakdown-table th {
      background-color: ${colors.headerBg};
      color: ${colors.headerText};
      padding: 10px;
      text-align: left;
      font-weight: 600;
      border-bottom: 2px solid ${colors.border};
    }

    .breakdown-table td {
      padding: 10px;
      border-bottom: 1px solid ${colors.border};
    }

    .breakdown-table tr:last-child td {
      border-bottom: none;
    }

    /* History Section */
    .history-section {
      margin-bottom: 40px;
    }

    .history-section h2 {
      font-size: 1.8em;
      color: ${colors.accent};
      margin-bottom: 20px;
      padding-bottom: 10px;
      border-bottom: 2px solid ${colors.accent};
    }

    .empty-message {
      color: ${colors.muted};
      font-style: italic;
      padding: 20px;
      text-align: center;
      background-color: ${colors.cardBg};
      border-radius: 4px;
    }

    /* Table Wrapper */
    .table-wrapper {
      overflow-x: auto;
      border: 1px solid ${colors.border};
      border-radius: 8px;
      background-color: ${colors.cardBg};
    }

    .history-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.95em;
    }

    .history-table thead {
      background-color: ${colors.headerBg};
      color: ${colors.headerText};
      position: sticky;
      top: 0;
    }

    .history-table th {
      padding: 12px;
      text-align: left;
      font-weight: 600;
      border-bottom: 2px solid ${colors.border};
    }

    .history-table td {
      padding: 12px;
      border-bottom: 1px solid ${colors.border};
    }

    .history-row:hover {
      background-color: ${colors.hoverBg};
    }

    .history-row.success {
      opacity: 1;
    }

    .history-row.error {
      opacity: 0.85;
    }

    /* Table Cell Styles */
    .date-time {
      white-space: nowrap;
      color: ${colors.muted};
      font-size: 0.9em;
    }

    .text-cell {
      max-width: 250px;
      word-wrap: break-word;
      color: ${colors.text};
    }

    .text-length {
      text-align: center;
      color: ${colors.muted};
    }

    .engine {
      font-weight: 500;
      color: ${colors.accent};
    }

    .voice {
      color: ${colors.text};
    }

    .speed {
      text-align: center;
      font-weight: 500;
    }

    .duration {
      text-align: center;
      color: ${colors.muted};
    }

    .format {
      text-align: center;
      color: ${colors.muted};
    }

    .status {
      text-align: center;
      font-weight: 600;
    }

    .status-success {
      color: #22c55e;
    }

    .status-error {
      color: #ef4444;
    }

    /* Tags */
    .tags {
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
    }

    .tag {
      display: inline-block;
      background-color: ${colors.tagBg};
      color: ${colors.tagText};
      padding: 3px 8px;
      border-radius: 12px;
      font-size: 0.85em;
    }

    /* Footer */
    .page-footer {
      border-top: 2px solid ${colors.border};
      padding-top: 20px;
      margin-top: 40px;
      text-align: center;
      color: ${colors.muted};
      font-size: 0.9em;
    }

    /* Print Styles */
    @media print {
      body {
        background-color: white;
        color: black;
      }

      .container {
        max-width: 100%;
      }

      .table-of-contents {
        page-break-inside: avoid;
      }

      .stat-card {
        page-break-inside: avoid;
      }

      .history-table {
        page-break-inside: avoid;
      }

      .history-row {
        page-break-inside: avoid;
      }
    }

    /* Responsive Design */
    @media (max-width: 768px) {
      .page-header h1 {
        font-size: 1.8em;
      }

      .stats-grid {
        grid-template-columns: 1fr;
      }

      .history-table {
        font-size: 0.85em;
      }

      .history-table th,
      .history-table td {
        padding: 8px;
      }

      .text-cell {
        max-width: 150px;
      }
    }
  `;
}

/**
 * Generates a compact HTML report (summary only)
 */
export function generateCompactHtmlReport(
  statistics: HistoryStatistics,
  itemCount: number
): string {
  const successPercentage = (statistics.success_rate * 100).toFixed(1);
  const totalDurationMinutes = (statistics.total_duration_ms / 1000 / 60).toFixed(1);

  return `
    <div class="compact-report">
      <h2>Summary Report</h2>
      <dl>
        <dt>Total Items:</dt>
        <dd>${itemCount}</dd>

        <dt>Successful:</dt>
        <dd>${statistics.successful_items} (${successPercentage}%)</dd>

        <dt>Failed:</dt>
        <dd>${statistics.failed_items}</dd>

        <dt>Total Duration:</dt>
        <dd>${totalDurationMinutes} minutes</dd>

        <dt>Average Text Length:</dt>
        <dd>${statistics.average_text_length.toFixed(0)} characters</dd>

        <dt>Most Used Voice:</dt>
        <dd>${statistics.most_used_voice || "N/A"}</dd>
      </dl>
    </div>`;
}

/**
 * Generates a timeline view of history items
 */
export function generateTimelineHtml(items: HistoryItem[]): string {
  if (items.length === 0) {
    return '<div class="timeline empty"><p>No items to display.</p></div>';
  }

  // Group items by date
  const itemsByDate = new Map<string, HistoryItem[]>();
  items.forEach((item) => {
    const date = formatHistoryDateISO(item.timestamp);
    if (!itemsByDate.has(date)) {
      itemsByDate.set(date, []);
    }
    itemsByDate.get(date)!.push(item);
  });

  const sortedDates = Array.from(itemsByDate.keys()).sort().reverse();

  return `
    <div class="timeline">
      ${sortedDates
        .map((date) => {
          const dateItems = itemsByDate.get(date)!;
          return `
        <div class="timeline-day">
          <h3 class="timeline-date">${escapeHtml(date)}</h3>
          <ul class="timeline-items">
            ${dateItems.map((item) => generateTimelineItemHtml(item)).join("")}
          </ul>
        </div>`;
        })
        .join("")}
    </div>`;
}

/**
 * Generates a single timeline item
 */
export function generateTimelineItemHtml(item: HistoryItem): string {
  const time = new Date(item.timestamp).toLocaleTimeString();
  const statusIcon = item.success ? "✓" : "✗";
  const statusClass = item.success ? "success" : "error";
  const textPreview =
    item.text.length > 80 ? escapeHtml(item.text.substring(0, 80)) + "..." : escapeHtml(item.text);

  return `
    <li class="timeline-item ${statusClass}">
      <span class="timeline-icon">${statusIcon}</span>
      <span class="timeline-time">${escapeHtml(time)}</span>
      <span class="timeline-engine">${escapeHtml(item.tts_engine)}</span>
      <p class="timeline-text">${textPreview}</p>
    </li>`;
}

/**
 * Escapes HTML special characters to prevent XSS
 */
export function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

/**
 * Color themes for HTML export
 * Re-exported from template backend for backward compatibility
 */
const LIGHT_THEME = LIGHT_THEME_COLORS;
const DARK_THEME = DARK_THEME_COLORS;

/**
 * Options for HTML export
 */
export interface HtmlExportOptions {
  title?: string;
  includeStatistics?: boolean;
  includeToc?: boolean;
  cssTheme?: "light" | "dark";
  dateRange?: {
    from: number;
    to: number;
  };
}

// Re-export template-related types and functions for convenience
export type { TemplateContext, ThemeColors } from "./html-templates";
export {
  LIGHT_THEME,
  DARK_THEME,
  createTemplateContext,
  renderHtmlTemplate
} from "./html-templates";
