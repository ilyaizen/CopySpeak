/**
 * Test file for HTML export and download functionality
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  exportHistoryToHtml,
  exportSelectedItemsToHtml,
  generateDefaultFilename,
  sanitizeFilename
} from "./html-export";
import type { HistoryItem, HistoryStatistics } from "$lib/types";

describe("HTML Export Utilities", () => {
  let mockUrl: string;
  let mockAnchor: HTMLAnchorElement;
  let originalCreateElement: typeof document.createElement;
  let originalAppendChild: typeof document.body.appendChild;
  let originalRemoveChild: typeof document.body.removeChild;

  beforeEach(() => {
    mockAnchor = document.createElement("a");
    mockAnchor.click = vi.fn();

    mockUrl = "blob:test-url";

    originalCreateElement = document.createElement;
    originalAppendChild = document.body.appendChild;
    originalRemoveChild = document.body.removeChild;

    document.createElement = vi.fn((tagName: string) => {
      if (tagName === "a") {
        return mockAnchor;
      }
      return originalCreateElement.call(document, tagName);
    }) as unknown as typeof document.createElement;

    document.body.appendChild = vi.fn() as unknown as typeof document.body.appendChild;
    document.body.removeChild = vi.fn() as unknown as typeof document.body.removeChild;

    globalThis.URL.createObjectURL = vi.fn(() => mockUrl);
    globalThis.URL.revokeObjectURL = vi.fn();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.createElement = originalCreateElement;
    document.body.appendChild = originalAppendChild;
    document.body.removeChild = originalRemoveChild;
  });

  describe("exportHistoryToHtml", () => {
    const mockHistoryItem: HistoryItem = {
      id: "test-1",
      timestamp: Date.now(),
      text: "Hello, world!",
      text_length: 13,
      tts_engine: "local",
      voice: "default",
      speed: 1.0,
      success: true,
      attempts: 1
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
      average_duration_ms: 1000
    };

    it("should generate and download HTML export", async () => {
      const progressCallback = vi.fn();

      await exportHistoryToHtml([mockHistoryItem], mockStatistics, {
        filename: "test-export",
        onProgress: progressCallback
      });

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "generating",
        percent: 0,
        message: "Generating HTML content..."
      });

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "generating",
        percent: 80,
        message: "HTML generated successfully"
      });

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "downloading",
        percent: 85,
        message: "Preparing download..."
      });

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "complete",
        percent: 100,
        message: "Export complete"
      });

      expect(URL.createObjectURL).toHaveBeenCalled();
      expect(mockAnchor.download).toBe("test-export.html");
      expect(mockAnchor.href).toBe(mockUrl);
      expect(mockAnchor.click).toHaveBeenCalled();
      expect(URL.revokeObjectURL).toHaveBeenCalledWith(mockUrl);
    });

    it("should handle empty history", async () => {
      const progressCallback = vi.fn();

      await exportHistoryToHtml([], undefined, {
        filename: "empty-export",
        onProgress: progressCallback
      });

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "complete",
        percent: 100,
        message: "Export complete"
      });
    });

    it("should use default filename when not provided", async () => {
      await exportHistoryToHtml([mockHistoryItem], mockStatistics);

      expect(mockAnchor.download).toMatch(/^copyspeak_history_\d{4}-\d{2}-\d{2}\.html$/);
    });

    it("should handle export errors", async () => {
      vi.spyOn(URL, "createObjectURL").mockImplementation(() => {
        throw new Error("Failed to create blob URL");
      });

      const progressCallback = vi.fn();

      await expect(
        exportHistoryToHtml([mockHistoryItem], mockStatistics, {
          onProgress: progressCallback
        })
      ).rejects.toThrow("Failed to create blob URL");

      expect(progressCallback).toHaveBeenCalledWith({
        stage: "error",
        percent: 0,
        message: "Export failed: Failed to create blob URL"
      });
    });

    it("should call progress callbacks in correct order", async () => {
      const progressCallback = vi.fn();

      await exportHistoryToHtml([mockHistoryItem], mockStatistics, {
        onProgress: progressCallback
      });

      const calls = progressCallback.mock.calls.map((call: any[]) => call[0]);
      expect(calls).toHaveLength(4);
      expect(calls[0].stage).toBe("generating");
      expect(calls[1].stage).toBe("generating");
      expect(calls[2].stage).toBe("downloading");
      expect(calls[3].stage).toBe("complete");
    });
  });

  describe("exportSelectedItemsToHtml", () => {
    const mockHistoryItem: HistoryItem = {
      id: "test-1",
      timestamp: Date.now(),
      text: "Selected item",
      text_length: 13,
      tts_engine: "local",
      voice: "default",
      speed: 1.0,
      success: true,
      attempts: 1
    };

    it("should export selected items with statistics", async () => {
      await exportSelectedItemsToHtml([mockHistoryItem], {
        filename: "selected-items"
      });

      expect(mockAnchor.download).toBe("selected-items.html");
      expect(mockAnchor.click).toHaveBeenCalled();
    });

    it("should handle empty selection", async () => {
      await exportSelectedItemsToHtml([], {
        filename: "empty-selection"
      });

      expect(mockAnchor.download).toBe("empty-selection.html");
      expect(mockAnchor.click).toHaveBeenCalled();
    });
  });

  describe("generateDefaultFilename", () => {
    it("should generate filename with current date", () => {
      const filename = generateDefaultFilename();
      const dateStr = new Date().toISOString().split("T")[0];
      expect(filename).toBe(`copyspeak_history_${dateStr}`);
    });

    it("should use custom prefix", () => {
      const filename = generateDefaultFilename("custom_export");
      const dateStr = new Date().toISOString().split("T")[0];
      expect(filename).toBe(`custom_export_${dateStr}`);
    });

    it("should use provided date", () => {
      const date = new Date("2024-01-15T10:30:00.000Z");
      const filename = generateDefaultFilename("export", date);
      expect(filename).toBe("export_2024-01-15");
    });
  });

  describe("sanitizeFilename", () => {
    it("should remove invalid characters", () => {
      const sanitized = sanitizeFilename('test<>:"/\\|?*file');
      expect(sanitized).toBe("test_________file");
    });

    it("should replace spaces with underscores", () => {
      const sanitized = sanitizeFilename("my file name");
      expect(sanitized).toBe("my_file_name");
    });

    it("should limit filename length", () => {
      const longFilename = "a".repeat(300);
      const sanitized = sanitizeFilename(longFilename);
      expect(sanitized.length).toBe(200);
    });

    it("should handle combined issues", () => {
      const sanitized = sanitizeFilename("my test<file> name with spaces");
      expect(sanitized).toBe("my_test_file__name_with_spaces");
    });
  });
});
