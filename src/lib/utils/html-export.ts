import type { HistoryItem, HistoryStatistics } from "$lib/types";
import {
	generateHistoryHtmlPage,
	generateCompactHtmlReport,
	type HtmlExportOptions
} from "$lib/models/history";

export interface ExportProgress {
	stage: "generating" | "downloading" | "complete" | "error";
	percent: number;
	message: string;
}

export interface HtmlExportProgressCallback {
	(progress: ExportProgress): void;
}

export async function exportHistoryToHtml(
	items: HistoryItem[],
	statistics?: HistoryStatistics,
	options?: {
		filename?: string;
		title?: string;
		includeStatistics?: boolean;
		includeToc?: boolean;
		cssTheme?: "light" | "dark";
		dateRange?: { from: number; to: number };
		onProgress?: HtmlExportProgressCallback;
	}
): Promise<void> {
	const {
		filename = `copyspeak_history_${new Date().toISOString().split("T")[0]}`,
		title = "CopySpeak History Export",
		includeStatistics = true,
		includeToc = items.length > 10,
		cssTheme = "light",
		dateRange,
		onProgress,
	} = options ?? {};

	try {
		onProgress?.({
			stage: "generating",
			percent: 0,
			message: "Generating HTML content..."
		});

		const htmlOptions: HtmlExportOptions = {
			title,
			includeStatistics,
			includeToc,
			cssTheme,
			dateRange,
		};

		let htmlContent: string;
		if (items.length === 0) {
			htmlContent = generateCompactHtmlReport(
				statistics ?? {
					total_items: 0,
					total_duration_ms: 0,
					successful_items: 0,
					failed_items: 0,
					success_rate: 0,
					by_engine: {} as Record<string, number>,
					by_format: {} as Record<string, number>,
					by_hour: {},
					by_day: {},
					most_used_voice: null,
					average_text_length: 0,
					average_duration_ms: 0,
				},
				0
			);
		} else {
			htmlContent = generateHistoryHtmlPage(items, statistics, htmlOptions);
		}

		onProgress?.({
			stage: "generating",
			percent: 80,
			message: "HTML generated successfully"
		});

		onProgress?.({
			stage: "downloading",
			percent: 85,
			message: "Preparing download..."
		});

		const blob = new Blob([htmlContent], { type: "text/html;charset=utf-8" });
		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = `${filename}.html`;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);
		URL.revokeObjectURL(url);

		onProgress?.({
			stage: "complete",
			percent: 100,
			message: "Export complete"
		});
	} catch (error) {
		onProgress?.({
			stage: "error",
			percent: 0,
			message: `Export failed: ${error instanceof Error ? error.message : "Unknown error"}`
		});
		throw error;
	}
}

export async function exportSelectedItemsToHtml(
	items: HistoryItem[],
	options?: {
		filename?: string;
		title?: string;
		includeStatistics?: boolean;
		includeToc?: boolean;
		cssTheme?: "light" | "dark";
		onProgress?: HtmlExportProgressCallback;
	}
): Promise<void> {
	const statistics = items.length > 0 ? calculateSelectedStatistics(items) : undefined;
	await exportHistoryToHtml(items, statistics, options);
}

function calculateSelectedStatistics(items: HistoryItem[]): HistoryStatistics {
	if (items.length === 0) {
		return {
			total_items: 0,
			total_duration_ms: 0,
			successful_items: 0,
			failed_items: 0,
			success_rate: 0,
			by_engine: {} as Record<string, number>,
			by_format: {} as Record<string, number>,
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
		by_engine: {} as Record<string, number>,
		by_format: {} as Record<string, number>,
		by_hour: {},
		by_day: {},
		most_used_voice: null,
		average_text_length: 0,
		average_duration_ms: 0,
	};

	const voiceCounts: Record<string, number> = {};
	let totalTextLength = 0;

	items.forEach((item) => {
		if (item.duration_ms) {
			stats.total_duration_ms += item.duration_ms;
		}

		if (item.success) {
			stats.successful_items++;
		} else {
			stats.failed_items++;
		}

		if (!stats.by_engine[item.tts_engine]) {
			stats.by_engine[item.tts_engine] = 0;
		}
		stats.by_engine[item.tts_engine]++;

		if (item.output_format) {
			if (!stats.by_format[item.output_format]) {
				stats.by_format[item.output_format] = 0;
			}
			stats.by_format[item.output_format]++;
		}

		const hour = new Date(item.timestamp).getHours();
		if (!stats.by_hour[hour]) {
			stats.by_hour[hour] = 0;
		}
		stats.by_hour[hour]++;

		const day = new Date(item.timestamp).toISOString().split("T")[0];
		if (!stats.by_day[day]) {
			stats.by_day[day] = 0;
		}
		stats.by_day[day]++;

		voiceCounts[item.voice] = (voiceCounts[item.voice] ?? 0) + 1;

		totalTextLength += item.text_length;
	});

	stats.average_text_length = totalTextLength / items.length;

	if (stats.total_duration_ms > 0) {
		stats.average_duration_ms = stats.total_duration_ms / items.length;
	}

	stats.success_rate = stats.successful_items / items.length;

	let maxCount = 0;
	for (const [voice, count] of Object.entries(voiceCounts)) {
		if (count > maxCount) {
			maxCount = count;
			stats.most_used_voice = voice;
		}
	}

	return stats;
}

export function generateDefaultFilename(
	prefix: string = "copyspeak_history",
	date?: Date
): string {
	const d = date ?? new Date();
	const dateStr = d.toISOString().split("T")[0];
	return `${prefix}_${dateStr}`;
}

export function sanitizeFilename(filename: string): string {
	return filename
		.replace(/[<>:"/\\|?*]/g, "_")
		.replace(/\s+/g, "_")
		.substring(0, 200);
}
