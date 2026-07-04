#!/usr/bin/env python3
"""CLI wrapper for KittenTTS - used by CopySpeak.

Reads text (inline or from a file), synthesizes with KittenTTS, and writes a
24kHz WAV file. Kept stable so CopySpeak's args_template never changes when
upstream shifts.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/kitten python scripts/copyspeak-kitten.py \
        --text-file {input} --voice {voice} --output {output}
"""

import argparse
import sys
from pathlib import Path


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    print("ERROR: provide --text or --text-file", file=sys.stderr)
    sys.exit(2)


def main() -> int:
    parser = argparse.ArgumentParser(description="KittenTTS CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument(
        "--voice",
        default="Rosie",
        help="Voice name. Options: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo",
    )
    parser.add_argument(
        "--model",
        default="KittenML/kitten-tts-nano-0.8",
        help="HuggingFace model id (default: kitten-tts-nano-0.8, 25MB)",
    )
    parser.add_argument("--output", required=True, help="Output WAV file path")
    args = parser.parse_args()

    text = read_text(args)

    try:
        from kittentts import KittenTTS
        import soundfile as sf
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-kittentts.ps1 -Force", file=sys.stderr)
        return 1

    try:
        tts = KittenTTS(args.model)
        audio = tts.generate(text=text, voice=args.voice, clean_text=True)
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        sf.write(args.output, audio, 24000)
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
