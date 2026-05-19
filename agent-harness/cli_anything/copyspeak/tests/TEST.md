# CopySpeak Harness Test Plan

## Test Inventory Plan

- `test_core.py`: 8 unit tests planned
- `test_full_e2e.py`: 5 E2E/subprocess tests planned

## Unit Test Plan

### `project.py` (4 tests)
- create default project schema/config
- save/load round trip
- reject invalid schema
- set config validates unknown keys

### `queue.py` (4 tests)
- add text item
- reject blank text
- remove existing item
- clear queue

## E2E Test Plan

- Create project JSON using installed CLI command.
- Add queue entries using subprocess.
- Export a text item through the real TTS backend.
- Verify output file exists, size > 0, and audio magic bytes are valid (`RIFF/WAVE`, `ID3`, `OggS`, or `fLaC`).
- Print artifact paths for manual inspection.

## Realistic Workflow Scenarios

### Quick clipboard-style phrase export
- Simulates: user copying a sentence and saving speech output.
- Operations chained: project new → queue add → export text.
- Verified: project JSON, queue state, real audio file format.

### Batch narration queue
- Simulates: agent preparing multiple snippets for playback/history.
- Operations chained: create project → add several queue items → export queue.
- Verified: multiple non-empty audio outputs and project history entries.

## Test Results

Not run yet. Per repository AGENTS.md, verification commands require explicit user confirmation.
