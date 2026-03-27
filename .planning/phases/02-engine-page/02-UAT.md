---
status: complete
phase: 02-engine-page
source: [02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md, 02-04-SUMMARY.md, 02-05-SUMMARY.md, 02-06-SUMMARY.md]
started: 2026-03-08T00:00:00Z
updated: 2026-03-08T00:10:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Engine Page Loads with 4 Backend Tabs
expected: Navigate to the Engine page. You should see 4 tabs: Local, HTTP, OpenAI, and ElevenLabs. The active tab should match your currently configured TTS backend.
result: pass

### 2. Local Engine Tab Configuration
expected: Click the Local tab. You should see fields for preset dropdown, command, args template, and voice input. Changing values in these fields should be possible.
result: pass

### 3. HTTP Engine Tab Configuration
expected: Click the HTTP tab. You should see fields for preset dropdown, URL template, body template, headers, timeout, and voice.
result: pass

### 4. OpenAI Engine Tab Configuration
expected: Click the OpenAI tab. You should see fields for API key, model selection, and voice dropdown.
result: pass

### 5. ElevenLabs Engine Tab Configuration
expected: Click the ElevenLabs tab. You should see fields for API key, voice dropdown (with fetch button), model, output format, and sliders for voice stability/similarity/style plus a speaker boost switch.
result: pass

### 6. Save Bar Appears on Changes
expected: On any engine tab, modify any field value. A save bar should appear at the bottom indicating unsaved changes, with Save and Discard buttons.
result: pass

### 7. Save Bar Disappears After Saving
expected: After the save bar appears (from test 6), click Save. The save bar should disappear and a success toast notification should appear confirming the config was saved.
result: pass <!-- CURRENT -->

### 8. Discard Reverts Changes
expected: Modify a field to trigger the save bar, then click Discard. The field should revert to its original value and the save bar should disappear.
result: pass <!-- CURRENT -->

### 9. Test Button Validates Engine
expected: With a valid engine configuration active, click the Test button (shared across all tabs). It should attempt to synthesize a test phrase and indicate success or failure.
result: pass <!-- CURRENT -->

### 10. TTS Section Removed from Settings
expected: Navigate to the Settings page. Confirm there is NO TTS or Engine configuration section — only General, Playback, Triggers, Sanitization, and History sections remain.
result: pass

## Summary

total: 10
passed: 10
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
