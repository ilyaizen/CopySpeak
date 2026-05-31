# Event System — History Updates

## Overview

`src/lib/utils/history-events.ts` provides utilities to listen for backend Tauri events and automatically refresh the frontend history store when changes occur.

## Events Listened

| Event             | Source                          | Action                          |
| ----------------- | ------------------------------- | ------------------------------- |
| `history-updated` | Backend (Rust)                  | Immediate history store refresh |
| `speak-request`   | Backend (clipboard double-copy) | 100ms debounced refresh         |

**Note:** Batch processing events (`batch:progress`) are deferred to `features-extras` branch.

## Usage

```typescript
import { startHistoryEventListeners, stopHistoryEventListeners } from "$lib/utils/history-events";

// In component mount
onMount(async () => {
  await startHistoryEventListeners();
});

// In component destroy
onDestroy(async () => {
  await stopHistoryEventListeners();
});
```

## Notes

- Listeners are deduplicated — calling `start` twice is a no-op.
- In non-Tauri environments (browser dev), all listeners are silently skipped.
- The global layout (`+layout.svelte`) starts listeners automatically.
- HUD window references have been removed (HUD feature is deferred to `features-extras` branch).
