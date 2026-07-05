#!/usr/bin/env python3
"""CLI wrapper for Piper (piper1-gpl) - used by CopySpeak.

Reads text (inline or from a file), loads the Piper voice model named by
--voice (resolved as ../voices/<voice>.onnx relative to this wrapper), synthesizes,
and writes a WAV file.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/piper python scripts/copyspeak-piper.py \
        --text-file {input} --voice {voice} --output {output}

Place <voice>.onnx + <voice>.onnx.json in <engine_dir>/voices/. Get voices
from https://github.com/OHF-Voice/piper1-gpl#voices
"""

import argparse
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


def main() -> int:
    parser = argparse.ArgumentParser(description="Piper CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument("--voice", default="en_US-joe-medium", help="Voice model basename in voices/")
    parser.add_argument("--output", required=True, help="Output WAV file path")
    args = parser.parse_args()

    text = read_text(args)

    # Wrapper lives in <engine_dir>/scripts/; voices live in <engine_dir>/voices/.
    voices_dir = Path(__file__).resolve().parent.parent / "voices"
    # ponytail: resolve by exact basename, else first .onnx whose stem ends with the voice name.
    model = (voices_dir / f"{args.voice}.onnx")
    if not model.exists():
        match = next((p for p in voices_dir.glob("*.onnx") if p.stem == args.voice), None)
        if match is None:
            print(f"ERROR: voice model not found: {model}", file=sys.stderr)
            print(f"  Available: {[p.stem for p in voices_dir.glob('*.onnx')]}", file=sys.stderr)
            return 1
        model = match

    try:
        from piper import PiperVoice
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-piper.ps1 -Force", file=sys.stderr)
        return 1

    try:
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        voice = PiperVoice.load(str(model))
        with wave.open(args.output, "wb") as wf:
            # ponytail: piper-tts 1.x owns WAV header setup via set_wav_format=True
            # (default); the 0.x `synthesize(wf, text)` signature is gone.
            voice.synthesize_wav(text, wf)
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
