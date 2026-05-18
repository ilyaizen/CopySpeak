---
name: "cli-anything-copyspeak"
description: "Operate CopySpeak desktop TTS from a stateful CLI: manage projects, queue text, invoke real TTS backends, and verify audio output."
---

# cli-anything-copyspeak

Use this skill when an agent needs to drive CopySpeak without the GUI.

## Install

```bash
cd /d/GitHub/CopySpeak/agent-harness
pip install -e .
```

Hard dependencies: real CopySpeak (`COPYSPEAK_EXE` or built Tauri executable) and a real TTS backend (`kittentts-cli.py`, Piper, Kokoro, etc.). Exports fail loudly if the backend is missing.

## Command pattern

Global flags come before command groups:

```bash
cli-anything-copyspeak --project project.json --json <group> <command>
```

Use `--json` for machine-readable output. Mutations auto-save; add `--dry-run` to suppress saving.

## Commands

- `project new -o FILE [--name NAME]` — create project JSON.
- `project info` — inspect current project.
- `project set-config --engine ENGINE --voice VOICE --speed N --pitch N --volume N` — update synthesis settings.
- `queue add -t TEXT [--label LABEL]` — enqueue text.
- `queue list` — list queued text.
- `queue remove ITEM_ID` — remove queued item.
- `queue clear` — clear queue.
- `export text -t TEXT -o AUDIO [--overwrite]` — synthesize one item using the real backend.
- `export queue -o DIR [--overwrite]` — synthesize all queued items.
- `backend check` — locate CopySpeak executable.
- `backend launch` — launch CopySpeak.

## Examples

```bash
cli-anything-copyspeak project new -o demo.json --name Demo
cli-anything-copyspeak --project demo.json queue add -t "Read this aloud" --label intro
cli-anything-copyspeak --project demo.json --json queue list
cli-anything-copyspeak --project demo.json export queue -o audio --overwrite
```

The export path validates non-empty audio and basic magic bytes. Do not treat success as only subprocess exit; inspect returned `output` and `file_size`.
