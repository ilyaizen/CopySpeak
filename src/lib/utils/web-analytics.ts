// Rybbit analytics for the web (landing) build only.
// +layout.svelte renders the tag inside its isWeb branch, so the Tauri app never loads it.
export const RYBBIT_HOST = "https://analytics.is-ai-good-yet.com";
export const COPYSPEECH_RYBBIT_SITE_ID = "8f8151b005e4";

// Rybbit reads the site id from the query string. The tracker takes its API base from
// the part of the src before "/script.js", so the path must stay /api/script.js.
export function rybbitScriptSrc(siteId: string): string {
  return `${RYBBIT_HOST}/api/script.js?siteId=${encodeURIComponent(siteId)}`;
}
