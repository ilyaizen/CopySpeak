#!/usr/bin/env python3
"""CLI wrapper for Piper (piper1-gpl) - used by CopySpeak.

Reads text (inline or from a file), loads the Piper voice model named by
--voice (resolved as ../voices/<voice>.onnx relative to this wrapper), synthesizes,
and writes a WAV file.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/piper python scripts/copyspeak-piper.py \
        --text-file {input} --voice {voice} --output {output}

With --serve the model is loaded once and stays in RAM: the wrapper prints
READY, then answers one JSON request per stdin line
({"text": ..., "output": ...}) with one JSON line
({"ok": true} or {"ok": false, "error": ...}). Stdin EOF ends the process,
so the daemon dies with CopySpeak.

Place <voice>.onnx + <voice>.onnx.json in <engine_dir>/voices/. Get voices
from https://github.com/OHF-Voice/piper1-gpl#voices
"""

import argparse
import json
import sys
import wave
from pathlib import Path


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    print("ERROR: provide --text or --text-file", file=sys.stderr)
    sys.exit(2)


def resolve_model(name: str) -> Path:
    # Wrapper lives in <engine_dir>/scripts/; voices live in <engine_dir>/voices/.
    voices_dir = Path(__file__).resolve().parent.parent / "voices"
    # ponytail: resolve by exact basename, else first .onnx whose stem ends with the voice name.
    model = voices_dir / f"{name}.onnx"
    if model.exists():
        return model
    match = next((p for p in voices_dir.glob("*.onnx") if p.stem == name), None)
    if match is None:
        available = [p.stem for p in voices_dir.glob("*.onnx")]
        raise FileNotFoundError(f"voice model not found: {model}\n  Available: {available}")
    return match


def synthesize(voice, text: str, output: str) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        # ponytail: piper-tts 1.x owns WAV header setup via set_wav_format=True
        # (default); the 0.x `synthesize(wf, text)` signature is gone.
        voice.synthesize_wav(text, wf)


def serve(voice) -> int:
    """Daemon mode: model stays resident, one JSON request per stdin line."""
    print("READY", flush=True)
    while True:
        line = sys.stdin.readline()
        if not line:
            return 0
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            synthesize(voice, request["text"], request["output"])
            reply = {"ok": True}
        except Exception as exc:  # pragma: no cover - environment dependent
            reply = {"ok": False, "error": str(exc)}
        print(json.dumps(reply), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Piper CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument("--voice", default="en_US-joe-medium", help="Voice model basename in voices/")
    parser.add_argument("--output", help="Output WAV file path (required unless --serve)")
    parser.add_argument("--serve", action="store_true", help="Keep the model in RAM and read JSON requests on stdin")
    args = parser.parse_args()

    if args.serve:
        text = None
    else:
        if not args.output:
            print("ERROR: --output is required unless --serve is given", file=sys.stderr)
            return 2
        text = read_text(args)

    try:
        model = resolve_model(args.voice)
    except FileNotFoundError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    try:
        from piper import PiperVoice
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-piper.ps1 -Force", file=sys.stderr)
        return 1

    try:
        voice = PiperVoice.load(str(model))
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    if args.serve:
        return serve(voice)

    try:
        synthesize(voice, text, args.output)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    print(f"OK -> {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
