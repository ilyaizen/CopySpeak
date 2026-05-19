# cli-anything-copyspeak

Stateful CLI harness for CopySpeak, a Windows Tauri desktop text-to-speech app.

## Requirements

- Python 3.10+
- Real CopySpeak app built or installed (`COPYSPEAK_EXE` may point to it)
- Real TTS backend for export: bundled `kittentts-cli.py` or installed local TTS CLI

The CLI does not reimplement rendering/synthesis in Python; `export` invokes a real backend and verifies the audio file.

## Install

```bash
cd agent-harness
pip install -e .
```

## Usage

```bash
cli-anything-copyspeak project new -o demo.json --name Demo
cli-anything-copyspeak --project demo.json queue add -t "Hello from CopySpeak" --label hello
cli-anything-copyspeak --project demo.json --json queue list
cli-anything-copyspeak --project demo.json export queue -o out --overwrite
cli-anything-copyspeak --json backend check
```

Run with no subcommand to open the REPL.

## Testing

```bash
CLI_ANYTHING_FORCE_INSTALLED=1 python -m pytest cli_anything/copyspeak/tests/ -v -s
```

E2E tests require real CopySpeak/TTS dependencies and fail if unavailable.
