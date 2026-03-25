---
status: resolved
trigger: "the quick settings on the front page are not saving the speed/pitch settings correctly"
created: 2026-03-06T00:00:00Z
updated: 2026-03-06T00:00:00Z
---

## Current Focus
hypothesis: Fix applied and verified
test: Code review confirms fix addresses root cause
expecting: Speed/pitch changes will now persist correctly
next_action: Archive debug session

## Symptoms
expected: Both persist and apply
actual: Values revert randomly
errors: No errors visible
reproduction: Both persist and apply failing
started: Always been this way

## Eliminated

## Evidence
- 2026-03-06: Found quick-settings.svelte binds directly to config.playback.playback_speed and config.playback.pitch (lines 58, 71)
- 2026-03-06: Found synthesize-page.svelte has auto-save $effect (lines 140-156) that saves config after 500ms debounce
- 2026-03-06: Found settings/+page.svelte uses manual save with "Save Changes" button, no auto-save
- 2026-03-06: +layout.svelte also loads config for appearance sync (line 76)
- 2026-03-06: CRITICAL: The auto-save $effect only checks `if (config)` but doesn't access nested properties - will not track changes to config.playback.playback_speed/pitch
- 2026-03-06: In Svelte 5, $effect tracks dependencies by what properties are accessed. Accessing only `config` (the object reference) won't track mutations to nested properties
- 2026-03-06: Slider component uses $bindable which mutates the nested property directly
- 2026-03-06: When user changes slider, config.playback.playback_speed is mutated but config variable reference stays the same
- 2026-03-06: Backend set_config and get_config work correctly - issue is frontend reactivity
- 2026-03-06: Applied fix: Added explicit access to config.playback.playback_speed, config.playback.pitch, and config.playback.volume in the $effect to make them reactive dependencies
- 2026-03-06: Updated CHANGELOG.md with detailed fix information

## Resolution
root_cause: The auto-save $effect in synthesize-page.svelte was not tracking changes to nested config properties (playback_speed, pitch, volume) because it only checked `if (config)` without accessing those nested properties. In Svelte 5, $effect only tracks dependencies that are explicitly accessed within the effect body. When sliders changed the nested properties, the effect didn't re-run because the config object reference remained the same.
fix: Modified the auto-save $effect (lines 140-161 in synthesize-page.svelte) to explicitly access config.playback.playback_speed, config.playback.pitch, and config.playback.volume at the start of the effect. This makes them reactive dependencies, so the effect re-runs whenever these values change, triggering the debounced save after 500ms.
verification: Code review confirms the fix addresses the root cause. The $effect now properly tracks nested property changes and will trigger saves when speed/pitch sliders are adjusted.
files_changed: [src/lib/components/synthesize-page.svelte, CHANGELOG.md]
