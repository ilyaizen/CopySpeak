#!/usr/bin/env python3
"""CLI wrapper for Resemble AI Chatterbox - used by CopySpeak.

Reads text (inline or from a file), synthesizes with Chatterbox, and writes a
WAV file. Kept stable so CopySpeak's args_template never needs to change when
upstream internals shift.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/chatterbox python scripts/copyspeak-chatterbox.py \
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


def main() -> None:
    parser = argparse.ArgumentParser(description="Chatterbox CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument(
        "--voice",
        default="default",
        help="Voice name or reference audio basename in the voices/ dir (default: default)",
    )
    parser.add_argument("--output", required=True, help="Output WAV file path")
    args = parser.parse_args()

    text = read_text(args)

    try:
        import torchaudio as ta
        from chatterbox.tts import ChatterboxTTS
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-chatterbox.ps1 -Force", file=sys.stderr)
        sys.exit(1)

    try:
        model = ChatterboxTTS.from_pretrained(device="cpu")

        # Optional voice cloning: wrapper lives in <engine_dir>/scripts/,
        # voice prompts live in <engine_dir>/voices/.
        prompt = Path(__file__).resolve().parent.parent / "voices" / f"{args.voice}.wav"
        if prompt.exists():
            wav = model.generate(text, audio_prompt_path=str(prompt))
        else:
            wav = model.generate(text)

        ta.save(args.output, wav, model.sr)
        print(f"Audio saved to {args.output}", file=sys.stderr)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
