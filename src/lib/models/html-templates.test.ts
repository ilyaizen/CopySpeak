/**
 * Test file to verify HTML template rendering functionality
 * This file demonstrates the usage of the template backend
 */

import { describe, it, expect } from "vitest";
import {
  createTemplateContext,
  renderHtmlTemplate,
  LIGHT_THEME,
  DARK_THEME,
} from "./html-templates";
import type { HistoryItem, HistoryStatistics } from "$lib/types";

describe("HTML Template Backend", () => {
  const mockHistoryItem: HistoryItem = {
    id: "test-1",
    timestamp: Date.now(),
    text: "Hello, world!",
    text_length: 13,
    tts_engine: "local",
    voice: "default",
    speed: 1.0,
    success: true,
    attempts: 1,
  };

  const mockStatistics: HistoryStatistics = {
    total_items: 1,
    total_duration_ms: 1000,
    successful_items: 1,
    failed_items: 0,
    success_rate: 1.0,
    by_engine: { local: 1, openai: 0, elevenlabs: 0 },
    by_format: { mp3: 1, wav: 0, ogg: 0, flac: 0 },
    by_hour: { 12: 1 },
    by_day: { "2024-01-01": 1 },
    most_used_voice: "default",
    average_text_length: 13,
    average_duration_ms: 1000,
  };

  it("should create a valid template context", () => {
    const context = createTemplateContext([mockHistoryItem], {
      title: "Test Export",
      theme: "light",
    });

    expect(context.title).toBe("Test Export");
    expect(context.items).toHaveLength(1);
    expect(context.theme).toEqual(LIGHT_THEME);
  });

  it("should render valid HTML", () => {
    const context = createTemplateContext([mockHistoryItem]);
    const html = renderHtmlTemplate(context);

    expect(html).toContain("<!DOCTYPE html>");
    expect(html).toContain("<html lang=\"en\">");
    expect(html).toContain("</html>");
    expect(html).toContain("Hello, world!");
  });

  it("should include statistics when provided", () => {
    const context = createTemplateContext([mockHistoryItem], {
      statistics: mockStatistics,
      includeStatistics: true,
    });

    const html = renderHtmlTemplate(context);

    expect(html).toContain("Statistics");
    expect(html).toContain("Total Items");
    expect(html).toContain("Success Rate");
  });

  it("should apply dark theme", () => {
    const context = createTemplateContext([mockHistoryItem], {
      theme: "dark",
    });

    const html = renderHtmlTemplate(context);

    // Check for dark theme colors
    expect(html).toContain(DARK_THEME.background);
    expect(html).toContain(DARK_THEME.text);
  });

  it("should escape HTML in user content", () => {
    const xssItem: HistoryItem = {
      ...mockHistoryItem,
      text: "<script>alert('xss')</script>",
    };

    const context = createTemplateContext([xssItem]);
    const html = renderHtmlTemplate(context);

    // Should NOT contain unescaped script tag
    expect(html).not.toContain("<script>alert('xss')</script>");
    // Should contain escaped version
    expect(html).toContain("&lt;script&gt;");
  });

  it("should include table of contents for many items", () => {
    const manyItems = Array.from({ length: 15 }, (_, i) => ({
      ...mockHistoryItem,
      id: `test-${i}`,
    }));

    const context = createTemplateContext(manyItems);
    const html = renderHtmlTemplate(context);

    expect(html).toContain("Table of Contents");
  });

  it("should NOT include table of contents for few items", () => {
    const context = createTemplateContext([mockHistoryItem], {
      includeToc: false,
    });

    const html = renderHtmlTemplate(context);

    expect(html).not.toContain('<nav class="table-of-contents">');
  });

  it("should handle empty history", () => {
    const context = createTemplateContext([]);
    const html = renderHtmlTemplate(context);

    expect(html).toContain("No items to display");
  });

  it("should render date range in header", () => {
    const context = createTemplateContext([mockHistoryItem], {
      dateRange: {
        from: Date.now() - 7 * 24 * 60 * 60 * 1000,
        to: Date.now(),
      },
    });

    const html = renderHtmlTemplate(context);

    expect(html).toContain("Date Range:");
  });
});
