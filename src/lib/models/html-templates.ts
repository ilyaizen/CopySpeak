/**
 * HTML template rendering system for history exports
 * Provides a clean separation between HTML structure and data
 */

import type { HistoryItem, HistoryStatistics } from "$lib/types";
import { formatHistoryDate, formatHistoryDateISO } from "./history";

/**
 * Template context for rendering HTML exports
 */
export interface TemplateContext {
  title: string;
  exportDate: string;
  dateRange?: {
    from: string;
    to: string;
  };
  statistics?: HistoryStatistics;
  items: HistoryItem[];
  theme: ThemeColors;
  includeStatistics: boolean;
  includeToc: boolean;
}

/**
 * Theme color configuration for HTML exports
 */
export interface ThemeColors {
  background: string;
  text: string;
  accent: string;
  link: string;
  muted: string;
  cardBg: string;
  border: string;
  headerBg: string;
  headerText: string;
  hoverBg: string;
  tagBg: string;
  tagText: string;
}

/**
 * Light theme color palette
 */
export const LIGHT_THEME: ThemeColors = {
  background: "#ffffff",
  text: "#1f2937",
  accent: "#3b82f6",
  link: "#2563eb",
  muted: "#6b7280",
  cardBg: "#f9fafb",
  border: "#e5e7eb",
  headerBg: "#f3f4f6",
  headerText: "#111827",
  hoverBg: "#f3f4f6",
  tagBg: "#dbeafe",
  tagText: "#1e40af"
};

/**
 * Dark theme color palette
 */
export const DARK_THEME: ThemeColors = {
  background: "#1f2937",
  text: "#f3f4f6",
  accent: "#60a5fa",
  link: "#93c5fd",
  muted: "#9ca3af",
  cardBg: "#111827",
  border: "#374151",
  headerBg: "#374151",
  headerText: "#f9fafb",
  hoverBg: "#374151",
  tagBg: "#1e3a8a",
  tagText: "#93c5fd"
};

/**
 * HTML template renderer
 */
export class HtmlTemplateRenderer {
  /**
   * Renders a complete HTML page using the provided template context
   */
  render(context: TemplateContext): string {
    const parts = [
      this.renderDocumentStart(context),
      this.renderHead(context),
      this.renderBodyStart(),
      this.renderContainer(context),
      this.renderBodyEnd(),
      this.renderDocumentEnd()
    ];

    return parts.join("\n");
  }

  /**
   * Renders document start
   */
  private renderDocumentStart(_context: TemplateContext): string {
    return "<!DOCTYPE html>";
  }

  /**
   * Renders document end
   */
  private renderDocumentEnd(): string {
    return "</html>";
  }

  /**
   * Renders the HTML head section
   */
  private renderHead(context: TemplateContext): string {
    return `<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${this.escape(context.title)}</title>
  <style>
    ${this.renderStyles(context.theme)}
  </style>
</head>`;
  }

  /**
   * Renders the body start tag
   */
  private renderBodyStart(): string {
    return "<body>";
  }

  /**
   * Renders the body end tag
   */
  private renderBodyEnd(): string {
    return "</body>";
  }

  /**
   * Renders the main container with all content
   */
  private renderContainer(context: TemplateContext): string {
    const parts = ['<div class="container">', this.renderPageHeader(context)];

    if (context.includeToc) {
      parts.push(this.renderTableOfContents(context));
    }

    if (context.includeStatistics && context.statistics) {
      parts.push(this.renderStatistics(context.statistics));
    }

    parts.push(this.renderHistoryTable(context.items));
    parts.push(this.renderPageFooter());
    parts.push("</div>");

    return parts.join("\n");
  }

  /**
   * Renders the page header section
   */
  private renderPageHeader(context: TemplateContext): string {
    let dateRangeHtml = "";
    if (context.dateRange) {
      dateRangeHtml = `<p>Date Range: <span class="date-range">${this.escape(context.dateRange.from)} to ${this.escape(context.dateRange.to)}</span></p>`;
    }

    return `
    <header class="page-header">
      <h1>${this.escape(context.title)}</h1>
      <div class="metadata">
        <p>Exported: <span class="timestamp">${this.escape(context.exportDate)}</span></p>
        ${dateRangeHtml}
      </div>
    </header>`;
  }

  /**
   * Renders the table of contents
   */
  private renderTableOfContents(context: TemplateContext): string {
    return `
    <nav class="table-of-contents">
      <h2>Table of Contents</h2>
      <ul>
        <li><a href="#statistics">Statistics</a></li>
        <li><a href="#history-items">History Items</a>
          <ul>
            <li>Total Items: ${context.items.length}</li>
            <li><a href="#by-engine">By Engine</a></li>
            <li><a href="#by-date">By Date</a></li>
          </ul>
        </li>
      </ul>
    </nav>`;
  }

  /**
   * Renders the statistics section
   */
  private renderStatistics(statistics: HistoryStatistics): string {
    const statCards = this.renderStatCards(statistics);
    const breakdowns = this.renderStatBreakdowns(statistics);

    return `
    <section id="statistics" class="statistics-section">
      <h2>Statistics</h2>
      <div class="stats-grid">
        ${statCards}
      </div>
      <div class="stats-breakdown">
        ${breakdowns}
      </div>
    </section>`;
  }

  /**
   * Renders statistics cards
   */
  private renderStatCards(statistics: HistoryStatistics): string {
    const successPercentage = (statistics.success_rate * 100).toFixed(1);
    const totalDurationMinutes = (statistics.total_duration_ms / 1000 / 60).toFixed(1);

    const cards = [
      {
        title: "Total Items",
        value: statistics.total_items.toString(),
        detail: ""
      },
      {
        title: "Success Rate",
        value: `${successPercentage}%`,
        detail: `${statistics.successful_items} successful, ${statistics.failed_items} failed`
      },
      {
        title: "Total Duration",
        value: `${totalDurationMinutes} min`,
        detail: `Avg: ${(statistics.average_duration_ms / 1000).toFixed(1)}s per item`
      },
      {
        title: "Average Text Length",
        value: statistics.average_text_length.toFixed(0),
        detail: "characters"
      }
    ];

    return cards
      .map(
        (card) => `
        <div class="stat-card">
          <h3>${card.title}</h3>
          <p class="stat-value">${card.value}</p>
          ${card.detail ? `<p class="stat-detail">${card.detail}</p>` : ""}
        </div>`
      )
      .join("");
  }

  /**
   * Renders statistics breakdowns
   */
  private renderStatBreakdowns(statistics: HistoryStatistics): string {
    const parts: string[] = [];

    // By Engine breakdown
    if (Object.keys(statistics.by_engine).length > 0) {
      parts.push(this.renderBreakdownTable("By TTS Engine", statistics.by_engine));
    }

    // By Format breakdown
    if (Object.keys(statistics.by_format).length > 0) {
      parts.push(this.renderBreakdownTable("By Format", statistics.by_format));
    }

    // Most used voice
    if (statistics.most_used_voice) {
      parts.push(`
        <div class="breakdown-section">
          <h3>Most Used Voice</h3>
          <p class="stat-detail">${this.escape(statistics.most_used_voice)}</p>
        </div>`);
    }

    return parts.join("\n");
  }

  /**
   * Renders a breakdown table
   */
  private renderBreakdownTable(title: string, data: Record<string, number>): string {
    const rows = Object.entries(data)
      .map(([key, count]) => `<tr><td>${this.escape(key)}</td><td>${count}</td></tr>`)
      .join("");

    return `
        <div class="breakdown-section">
          <h3>${title}</h3>
          <table class="breakdown-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Count</th>
              </tr>
            </thead>
            <tbody>
              ${rows}
            </tbody>
          </table>
        </div>`;
  }

  /**
   * Renders the history items table
   */
  private renderHistoryTable(items: HistoryItem[]): string {
    if (items.length === 0) {
      return `
      <section id="history-items" class="history-section">
        <h2>History Items</h2>
        <p class="empty-message">No items to display.</p>
      </section>`;
    }

    const rows = items.map((item) => this.renderHistoryRow(item)).join("");

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
            ${rows}
          </tbody>
        </table>
      </div>
    </section>`;
  }

  /**
   * Renders a single history item row
   */
  private renderHistoryRow(item: HistoryItem): string {
    const dateTime = formatHistoryDate(item.timestamp);
    const duration = item.duration_ms ? `${(item.duration_ms / 1000).toFixed(1)}s` : "—";
    const format = item.output_format || "—";
    const status = item.success
      ? '<span class="status-success">✓ Success</span>'
      : '<span class="status-error">✗ Failed</span>';

    const tags = item.tags
      ? item.tags.map((tag) => `<span class="tag">${this.escape(tag)}</span>`).join(" ")
      : "—";

    const textPreview =
      item.text.length > 100
        ? this.escape(item.text.substring(0, 100)) + "..."
        : this.escape(item.text);

    const rowClass = item.success ? "success" : "error";

    return `
    <tr class="history-row ${rowClass}">
      <td class="date-time">${this.escape(dateTime)}</td>
      <td class="text-cell" title="${this.escape(item.text)}">${textPreview}</td>
      <td class="text-length">${item.text_length}</td>
      <td class="engine">${this.escape(item.tts_engine)}</td>
      <td class="voice">${this.escape(item.voice)}</td>
      <td class="speed">${item.speed.toFixed(2)}x</td>
      <td class="duration">${duration}</td>
      <td class="format">${this.escape(format)}</td>
      <td class="status">${status}</td>
      <td class="tags">${tags}</td>
    </tr>`;
  }

  /**
   * Renders the page footer
   */
  private renderPageFooter(): string {
    const timestamp = new Date().toISOString();
    return `
    <footer class="page-footer">
      <p>Generated by CopySpeak - ${this.escape(timestamp)}</p>
    </footer>`;
  }

  /**
   * Renders CSS styles for the HTML export
   */
  private renderStyles(theme: ThemeColors): string {
    return `
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }

    html, body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
      background-color: ${theme.background};
      color: ${theme.text};
      line-height: 1.6;
    }

    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 20px;
    }

    /* Header Styles */
    .page-header {
      border-bottom: 3px solid ${theme.accent};
      padding-bottom: 20px;
      margin-bottom: 30px;
    }

    .page-header h1 {
      font-size: 2.5em;
      color: ${theme.accent};
      margin-bottom: 10px;
    }

    .metadata {
      color: ${theme.muted};
      font-size: 0.95em;
    }

    .metadata p {
      margin: 5px 0;
    }

    /* Table of Contents */
    .table-of-contents {
      background-color: ${theme.cardBg};
      border-left: 4px solid ${theme.accent};
      padding: 15px;
      margin-bottom: 30px;
      border-radius: 4px;
    }

    .table-of-contents h2 {
      font-size: 1.3em;
      color: ${theme.accent};
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
      color: ${theme.link};
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
      color: ${theme.accent};
      margin-bottom: 20px;
      padding-bottom: 10px;
      border-bottom: 2px solid ${theme.accent};
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 20px;
      margin-bottom: 30px;
    }

    .stat-card {
      background-color: ${theme.cardBg};
      border: 1px solid ${theme.border};
      border-radius: 8px;
      padding: 20px;
      text-align: center;
    }

    .stat-card h3 {
      font-size: 0.95em;
      color: ${theme.muted};
      text-transform: uppercase;
      margin-bottom: 10px;
      letter-spacing: 0.5px;
    }

    .stat-value {
      font-size: 2.5em;
      font-weight: bold;
      color: ${theme.accent};
      margin: 10px 0;
    }

    .stat-detail {
      font-size: 0.9em;
      color: ${theme.muted};
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
      background-color: ${theme.cardBg};
      border: 1px solid ${theme.border};
      border-radius: 8px;
      padding: 20px;
    }

    .breakdown-section h3 {
      color: ${theme.accent};
      margin-bottom: 15px;
      font-size: 1.1em;
    }

    .breakdown-table {
      width: 100%;
      border-collapse: collapse;
    }

    .breakdown-table th {
      background-color: ${theme.headerBg};
      color: ${theme.headerText};
      padding: 10px;
      text-align: left;
      font-weight: 600;
      border-bottom: 2px solid ${theme.border};
    }

    .breakdown-table td {
      padding: 10px;
      border-bottom: 1px solid ${theme.border};
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
      color: ${theme.accent};
      margin-bottom: 20px;
      padding-bottom: 10px;
      border-bottom: 2px solid ${theme.accent};
    }

    .empty-message {
      color: ${theme.muted};
      font-style: italic;
      padding: 20px;
      text-align: center;
      background-color: ${theme.cardBg};
      border-radius: 4px;
    }

    /* Table Wrapper */
    .table-wrapper {
      overflow-x: auto;
      border: 1px solid ${theme.border};
      border-radius: 8px;
      background-color: ${theme.cardBg};
    }

    .history-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.95em;
    }

    .history-table thead {
      background-color: ${theme.headerBg};
      color: ${theme.headerText};
      position: sticky;
      top: 0;
    }

    .history-table th {
      padding: 12px;
      text-align: left;
      font-weight: 600;
      border-bottom: 2px solid ${theme.border};
    }

    .history-table td {
      padding: 12px;
      border-bottom: 1px solid ${theme.border};
    }

    .history-row:hover {
      background-color: ${theme.hoverBg};
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
      color: ${theme.muted};
      font-size: 0.9em;
    }

    .text-cell {
      max-width: 250px;
      word-wrap: break-word;
      color: ${theme.text};
    }

    .text-length {
      text-align: center;
      color: ${theme.muted};
    }

    .engine {
      font-weight: 500;
      color: ${theme.accent};
    }

    .voice {
      color: ${theme.text};
    }

    .speed {
      text-align: center;
      font-weight: 500;
    }

    .duration {
      text-align: center;
      color: ${theme.muted};
    }

    .format {
      text-align: center;
      color: ${theme.muted};
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
      background-color: ${theme.tagBg};
      color: ${theme.tagText};
      padding: 3px 8px;
      border-radius: 12px;
      font-size: 0.85em;
    }

    /* Footer */
    .page-footer {
      border-top: 2px solid ${theme.border};
      padding-top: 20px;
      margin-top: 40px;
      text-align: center;
      color: ${theme.muted};
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
   * Escapes HTML special characters to prevent XSS
   */
  private escape(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }
}

/**
 * Creates a template context from provided data
 */
export function createTemplateContext(
  items: HistoryItem[],
  options: {
    title?: string;
    statistics?: HistoryStatistics;
    includeStatistics?: boolean;
    includeToc?: boolean;
    theme?: "light" | "dark";
    dateRange?: { from: number; to: number };
  } = {}
): TemplateContext {
  const {
    title = "CopySpeak History Export",
    statistics,
    includeStatistics = true,
    includeToc = items.length > 10,
    theme = "light",
    dateRange
  } = options;

  const themeColors = theme === "light" ? LIGHT_THEME : DARK_THEME;

  const exportDate = new Date().toLocaleString();

  let dateRangeFormatted: { from: string; to: string } | undefined;
  if (dateRange) {
    dateRangeFormatted = {
      from: formatHistoryDateISO(dateRange.from),
      to: formatHistoryDateISO(dateRange.to)
    };
  }

  return {
    title,
    exportDate,
    dateRange: dateRangeFormatted,
    statistics,
    items,
    theme: themeColors,
    includeStatistics,
    includeToc
  };
}

/**
 * Renders HTML using the template system
 */
export function renderHtmlTemplate(context: TemplateContext): string {
  const renderer = new HtmlTemplateRenderer();
  return renderer.render(context);
}
