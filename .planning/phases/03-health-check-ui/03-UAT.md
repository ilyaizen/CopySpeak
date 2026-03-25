---
status: testing
phase: 03-health-check-ui
source: 03-01-SUMMARY.md
started: 2026-03-08T00:00:00Z
updated: 2026-03-08T00:00:00Z
---

## Current Test

number: 1
name: Health Check UI Appears in All Backends
expected: |
  Navigate to the Engine page. For each of the 4 backend types (Local CLI, HTTP Server, OpenAI, ElevenLabs), you should see a test button labeled something like "Test Connection" or "Health Check" in that backend's configuration section.
awaiting: user response

## Tests

### 1. Health Check UI Appears in All Backends
expected: Test button visible in all 4 backend components (Local, HTTP, OpenAI, ElevenLabs)
result: [pending]

### 2. Test Button Only Shows When Backend is Active
expected: When you switch to a different backend, the test button for the previously active backend disappears, and only the current backend's test button is visible
result: [pending]

### 3. Health Check Test Executes
expected: Clicking the test button triggers a health check. A result should appear (either success or error message) within a few seconds
result: [pending]

### 4. Success Result Shows Green Alert
expected: When the backend test succeeds (backend is properly configured and working), a green alert banner appears with a checkmark icon and a success message
result: [pending]

### 5. Error Result Shows Red Alert with Error Message
expected: When the backend test fails, a red alert banner appears with an X icon and a human-readable error message (not raw error code) explaining what went wrong
result: [pending]

### 6. Install Guidance for CLI Backends
expected: If testing a local CLI backend (like local TTS) and it fails due to missing CLI tool, an install guidance card appears below the alert showing the command to install the required dependency, with a copy button to copy the command
result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0

## Gaps

[none yet]
