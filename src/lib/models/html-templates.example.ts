/**
 * Example usage of the HTML Template Backend
 *
 * This file demonstrates various ways to use the template rendering system
 * for creating HTML exports of history data.
 */

import {
  createTemplateContext,
  renderHtmlTemplate,
  LIGHT_THEME,
  type TemplateContext,
  type ThemeColors,
} from "./html-templates";
import type { HistoryItem, HistoryStatistics } from "$lib/types";

// Example 1: Basic HTML Export
export function basicHtmlExport(items: HistoryItem[]): string {
  const context = createTemplateContext(items, {
    title: "My History Export",
  });

  return renderHtmlTemplate(context);
}

// Example 2: Export with Statistics
export function exportWithStatistics(
  items: HistoryItem[],
  statistics: HistoryStatistics
): string {
  const context = createTemplateContext(items, {
    title: "History Report with Statistics",
    statistics,
    includeStatistics: true,
  });

  return renderHtmlTemplate(context);
}

// Example 3: Dark Theme Export
export function darkThemeExport(items: HistoryItem[]): string {
  const context = createTemplateContext(items, {
    title: "Dark Theme Export",
    theme: "dark",
  });

  return renderHtmlTemplate(context);
}

// Example 4: Custom Theme Export
export function customThemeExport(items: HistoryItem[]): string {
  // Create a custom theme based on the light theme
  const customTheme: ThemeColors = {
    ...LIGHT_THEME,
    accent: "#ff6b6b", // Custom red accent
    background: "#fffef9", // Warm white background
    cardBg: "#fff9f0", // Warm card background
  };

  const context = createTemplateContext(items, {
    title: "Custom Theme Export",
  });

  // Override the theme
  context.theme = customTheme;

  return renderHtmlTemplate(context);
}

// Example 5: Date Range Export
export function dateRangeExport(
  items: HistoryItem[],
  fromDate: Date,
  toDate: Date
): string {
  const context = createTemplateContext(items, {
    title: "Date Range Report",
    dateRange: {
      from: fromDate.getTime(),
      to: toDate.getTime(),
    },
  });

  return renderHtmlTemplate(context);
}

// Example 6: Export Without Table of Contents
export function compactExport(items: HistoryItem[]): string {
  const context = createTemplateContext(items, {
    title: "Compact Report",
    includeToc: false,
  });

  return renderHtmlTemplate(context);
}

// Example 7: Export Without Statistics
export function dataOnlyExport(items: HistoryItem[]): string {
  const context = createTemplateContext(items, {
    title: "Data Only Export",
    includeStatistics: false,
  });

  return renderHtmlTemplate(context);
}

// Example 8: Weekly Report
export function weeklyReport(
  items: HistoryItem[],
  statistics: HistoryStatistics
): string {
  const now = new Date();
  const weekAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);

  const context = createTemplateContext(items, {
    title: "Weekly TTS Report",
    statistics,
    includeStatistics: true,
    includeToc: items.length > 10,
    theme: "light",
    dateRange: {
      from: weekAgo.getTime(),
      to: now.getTime(),
    },
  });

  return renderHtmlTemplate(context);
}

// Example 9: Monthly Summary
export function monthlySummary(
  items: HistoryItem[],
  statistics: HistoryStatistics,
  month: number,
  year: number
): string {
  const startDate = new Date(year, month, 1);
  const endDate = new Date(year, month + 1, 0);

  const monthNames = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];

  const context = createTemplateContext(items, {
    title: `${monthNames[month]} ${year} - TTS Summary`,
    statistics,
    includeStatistics: true,
    includeToc: true,
    theme: "light",
    dateRange: {
      from: startDate.getTime(),
      to: endDate.getTime(),
    },
  });

  return renderHtmlTemplate(context);
}

// Example 10: Custom HTML Export with Additional Content
export function customBrandedExport(items: HistoryItem[]): string {
  const context = createTemplateContext(items, {
    title: "Custom Branded Export",
  });

  const baseHtml = renderHtmlTemplate(context);

  // Add custom branding to the HTML
  const customBranding = `
    <div class="custom-branding" style="text-align: center; margin: 20px 0;">
      <img src="https://example.com/logo.png" alt="Company Logo" style="max-width: 200px;">
    </div>
  `;

  const watermark = `
    <div class="watermark" style="text-align: center; margin-top: 10px; opacity: 0.5;">
      <p>Confidential - Internal Use Only</p>
    </div>
  `;

  // Insert branding after header and watermark before footer
  return baseHtml
    .replace('</header>', `</header>${customBranding}`)
    .replace('</footer>', `${watermark}</footer>`);
}

// Example 11: Using Custom HTML Export
export function customRendererExport(items: HistoryItem[]): string {
  return customBrandedExport(items);
}

// Example 12: Batch Export (Multiple Files)
export function batchExport(
  allItems: HistoryItem[],
  itemsPerFile: number = 100
): string[] {
  const htmlFiles: string[] = [];
  const totalFiles = Math.ceil(allItems.length / itemsPerFile);

  for (let i = 0; i < totalFiles; i++) {
    const start = i * itemsPerFile;
    const end = Math.min(start + itemsPerFile, allItems.length);
    const batch = allItems.slice(start, end);

    const context = createTemplateContext(batch, {
      title: `History Export - Part ${i + 1} of ${totalFiles}`,
      includeToc: false,
      includeStatistics: false,
    });

    htmlFiles.push(renderHtmlTemplate(context));
  }

  return htmlFiles;
}

// Example 13: Export for Printing
export function printOptimizedExport(
  items: HistoryItem[],
  statistics: HistoryStatistics
): string {
  const context = createTemplateContext(items, {
    title: "Print Report",
    statistics,
    includeStatistics: true,
    includeToc: true,
    theme: "light", // Light theme is better for printing
  });

  return renderHtmlTemplate(context);
}

// Example 14: Email-Ready HTML
export function emailReadyExport(
  items: HistoryItem[],
  statistics: HistoryStatistics
): string {
  // For emails, keep it simple - no TOC, compact layout
  const context = createTemplateContext(items.slice(0, 20), {
    // Limit to 20 items
    title: "TTS Activity Summary",
    statistics,
    includeStatistics: true,
    includeToc: false,
    theme: "light",
  });

  return renderHtmlTemplate(context);
}

// Example 15: Integration with Download Function
export function downloadHtmlExport(
  items: HistoryItem[],
  filename: string = "history-export.html"
): void {
  const html = basicHtmlExport(items);

  // Create blob and download
  const blob = new Blob([html], { type: "text/html;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// Example 16: Preview in New Window
export function previewHtmlExport(items: HistoryItem[]): void {
  const html = basicHtmlExport(items);

  const newWindow = window.open("", "_blank");
  if (newWindow) {
    newWindow.document.write(html);
    newWindow.document.close();
  }
}

// Example 17: Template Context Reuse
export function reuseTemplateContext(
  baseContext: TemplateContext,
  modifications: Partial<TemplateContext>
): string {
  // Create a new context based on existing one
  const newContext: TemplateContext = {
    ...baseContext,
    ...modifications,
  };

  return renderHtmlTemplate(newContext);
}

// Example 18: Filter and Export
export function filterAndExport(
  items: HistoryItem[],
  filter: (item: HistoryItem) => boolean,
  title: string
): string {
  const filteredItems = items.filter(filter);

  const context = createTemplateContext(filteredItems, {
    title,
  });

  return renderHtmlTemplate(context);
}

// Example Usage Scenarios:

/*
// Scenario 1: Export today's history
const todayItems = items.filter(item => {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return item.timestamp >= today.getTime();
});
const html = basicHtmlExport(todayItems);

// Scenario 2: Export failed items only
const failedItems = items.filter(item => !item.success);
const html = filterAndExport(failedItems, () => true, "Failed TTS Attempts");

// Scenario 3: Export by engine
const openaiItems = items.filter(item => item.tts_engine === "openai");
const html = basicHtmlExport(openaiItems);

// Scenario 4: Create weekly reports
const lastWeek = items.filter(item => {
  const weekAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
  return item.timestamp >= weekAgo;
});
const html = weeklyReport(lastWeek, statistics);

// Scenario 5: Custom branded export for clients
const html = customRendererExport(items);
downloadHtmlExport(items, "client-report.html");
*/
