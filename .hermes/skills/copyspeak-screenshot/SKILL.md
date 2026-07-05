---
name: copyspeak-screenshot
description: Capture a fresh screenshot of the CopySpeak Tauri app window for the README and landing page. Use when the screenshot is stale, after UI changes, or before a release.
---

# CopySpeak Screenshot Capture

Capture a screenshot of the running CopySpeak Tauri window and update the README/landing page reference.

## Prerequisites

1. **Tauri dev must be running**: `bun run tauri dev` (started from project root)
2. **CopySpeak window must be visible** — not minimized to tray
3. **Window must be on the Play page** (default route `/`) — this is what the screenshot should show

The browser-only dev server (`bun run dev` → `http://localhost:5173/`) will NOT work — the Play page calls Tauri IPC (`invoke()`) which only exists inside the Tauri webview. Must use the full Tauri app.

## Quick Capture

```bash
cd D:\GitHub\CopySpeak
node scripts/capture-screenshot.mjs
```

This script:
1. Reads version from `src-tauri/tauri.conf.json`
2. Captures the window via `scripts/screenshot-window.ps1`
3. Saves to `static/screen-v{version}.png`
4. Patches `src/lib/components/landing/screenshots.svelte` to reference the new file

Add `--no-update` to capture only without patching the Svelte reference.

## Manual Capture (debugging)

```bash
# List all visible windows (find the right title)
pwsh -NoProfile -File scripts/screenshot-window.ps1 -List

# Capture to a custom path
pwsh -NoProfile -File scripts/screenshot-window.ps1 -WindowTitle "CopySpeak" -OutPath "static/screen-test.png"
```

## Key Details

- **Window title**: `CopySpeak` (just the product name, NOT "CopySpeak TTS")
- **Tauri window size**: 775×580 (from `tauri.conf.json`) — screenshot captures the DWM extended frame bounds, so actual capture is slightly larger (~777×612)
- **Output location**: `static/` directory (served by SvelteKit at `/`)
- **Referenced in**: `src/lib/components/landing/screenshots.svelte` — the landing page screenshots section
- **Vercel landing page**: Only renders when `VITE_IS_VERCEL=true` — the screenshot is shown in the `<Screenshots />` component there
- **Requires**: `pwsh` (PowerShell 7). If unavailable, use `powershell.exe` instead.

## After Version Bump

When you bump the version (`bun run bump`, `bun run bump:minor`, `bun run bump:major`), the old screenshot file remains in `static/`. The capture script creates a new `screen-v{new_version}.png`. Optionally delete the old file:

```bash
cd D:\GitHub\CopySpeak\static
ls screen-v*.png   # review before deleting
rm screen-v0.1.6.png   # old version
```

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "No visible window title contains 'CopySpeak'" | Tauri dev not running, or window minimized to tray. Run `bun run tauri dev`, make sure window is visible. |
| Screenshot shows wrong page | Navigate to Play page (default route `/`) in the app before capturing. |
| Screenshot captures VS Code or another window | Multiple windows match "CopySpeak" substring. Close other matching windows or use a more specific title. |
| Play page looks broken/empty | You're seeing the browser dev server, not Tauri. The Play page needs Tauri IPC. Run `bun run tauri dev`. |
| pwsh not found | Use `powershell.exe` instead, or install PowerShell 7: `winget install Microsoft.PowerShell` |
