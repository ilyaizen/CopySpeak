import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Opens a URL in the system's default browser.
 * Use this instead of <a target="_blank"> to properly open links
 * outside of Tauri's webview.
 */
export async function openExternal(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    console.error("Failed to open external URL:", error);
    // Fallback: try to open via window.open (may not work in Tauri)
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
