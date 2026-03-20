#!/usr/bin/env python3
"""
CLI wrapper for KittenTTS - used by CopySpeak application.

KittenTTS is an ultra-lightweight TTS model (15M-80M parameters, 25-80MB on disk)
that runs on CPU without requiring a GPU.

Usage:
    python kittentts-cli.py --text "Hello world" --voice Jasper --output output.wav
    python kittentts-cli.py --text "Hello" --voice Luna --output out.wav --model KittenML/kitten-tts-micro-0.8

Available models:
    KittenML/kitten-tts-nano-0.8    - 15M params, 25MB (fastest, smallest)
    KittenML/kitten-tts-micro-0.8   - 40M params, 41MB (balanced)
    KittenML/kitten-tts-mini-0.8    - 80M params, 80MB (highest quality)

Available voices:
    Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo

Installation:
    pip install https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl
    pip install soundfile
"""

import argparse
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(
        description="KittenTTS CLI wrapper for CopySpeak",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --text "Hello world" --voice Jasper --output speech.wav
  %(prog)s --text "Hi there" --voice Luna --output out.wav --model KittenML/kitten-tts-micro-0.8
        """,
    )
    parser.add_argument("--text", required=True, help="Text to synthesize")
    parser.add_argument(
        "--voice",
        default="Jasper",
        help="Voice name (default: Jasper). Options: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo",
    )
    parser.add_argument("--output", required=True, help="Output WAV file path")
    parser.add_argument(
        "--model",
        default="KittenML/kitten-tts-nano-0.8",
        help="Model name (default: KittenML/kitten-tts-nano-0.8)",
    )
    parser.add_argument(
        "--list-voices", action="store_true", help="List available voices and exit"
    )

    args = parser.parse_args()

    if args.list_voices:
        print("Available voices: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo")
        return 0

    try:
        from kittentts import KittenTTS
        import soundfile as sf
    except ImportError as e:
        print(f"ERROR: Missing dependency: {e}", file=sys.stderr)
        print("", file=sys.stderr)
        print("Install KittenTTS with:", file=sys.stderr)
        print(
            "  pip install https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl",
            file=sys.stderr,
        )
        print("  pip install soundfile", file=sys.stderr)
        return 1

    try:
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        tts = KittenTTS(args.model)
        audio = tts.generate(text=args.text, voice=args.voice, clean_text=True)
        sf.write(str(output_path), audio, 24000)

        print(f"OK: {len(audio)} samples -> {output_path}", file=sys.stderr)
        return 0

    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
