#!/usr/bin/env python3
"""
HTTP server for KittenTTS — keeps the model loaded in RAM.

KittenTTS models are ultra-lightweight (25-80MB) but loading them on every
utterance still costs ~500ms-2s. This server loads once and reuses.

Usage:
    python kitten_server.py --port 51235
    python kitten_server.py --port 51235 --model KittenML/kitten-tts-micro-0.8

Install deps:
    pip install https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl
    pip install soundfile numpy

Endpoints (mirror Piper HTTP server contract):
    POST /          {"text": "...", "voice": "Rosie", "length_scale": 1.0}  → WAV bytes
    GET  /voices     → {"voices": [...]}   (also used as health check)
"""

import argparse
import io
import json
import sys
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler

# ---------------------------------------------------------------------------
# Hardcoded voice list (matches CopySpeak frontend: local-engine.svelte)
# ---------------------------------------------------------------------------
KITTEN_VOICES = [
    "Bella",
    "Jasper",
    "Luna",
    "Bruno",
    "Rosie",
    "Hugo",
    "Kiki",
    "Leo",
]

DEFAULT_MODEL = "KittenML/kitten-tts-nano-0.8"
DEFAULT_VOICE = "Rosie"
SAMPLE_RATE = 24000

# ---------------------------------------------------------------------------
# Globals set at startup
# ---------------------------------------------------------------------------
TTS = None
AVAILABLE_VOICES = KITTEN_VOICES[:]


def load_model(model_name=DEFAULT_MODEL):
    """Load KittenTTS model once."""
    from kittentts import KittenTTS

    tts = KittenTTS(model_name)
    return tts


def synthesize(text, voice, length_scale):
    """Run inference, return WAV bytes."""
    import numpy as np
    import soundfile as sf

    # KittenTTS doesn't expose a speed parameter at synthesis time;
    # length_scale is accepted for API compatibility but ignored.
    _ = length_scale

    audio = TTS.generate(text=text, voice=voice, clean_text=True)

    # KittenTTS returns a list of float32 samples
    audio_array = np.array(audio, dtype=np.float32)

    buf = io.BytesIO()
    sf.write(buf, audio_array, SAMPLE_RATE, format="WAV")
    return buf.getvalue()


# ---------------------------------------------------------------------------
# HTTP handler
# ---------------------------------------------------------------------------
class KittenHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[kitten_server] {args[0]}", file=sys.stderr, flush=True)

    def do_GET(self):
        if self.path in ("/voices", "/health", "/"):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            body = json.dumps({"voices": AVAILABLE_VOICES, "default": DEFAULT_VOICE})
            self.wfile.write(body.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            req = json.loads(body)

            text = req.get("text", "")
            voice = req.get("voice", DEFAULT_VOICE)
            length_scale = req.get("length_scale", 1.0)

            if not text.strip():
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"empty text")
                return

            if voice not in AVAILABLE_VOICES:
                print(
                    f"[kitten_server] Unknown voice '{voice}', falling back to {DEFAULT_VOICE}",
                    file=sys.stderr,
                    flush=True,
                )
                voice = DEFAULT_VOICE

            wav_bytes = synthesize(text, voice, length_scale)

            self.send_response(200)
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Content-Length", str(len(wav_bytes)))
            self.end_headers()
            self.wfile.write(wav_bytes)

        except Exception:
            self.send_response(500)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            tb = traceback.format_exc()
            print(f"[kitten_server] Synthesis error:\n{tb}", file=sys.stderr, flush=True)
            self.wfile.write(tb.encode())


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="KittenTTS HTTP server")
    parser.add_argument("--port", type=int, required=True, help="TCP port to listen on")
    parser.add_argument("--host", default="127.0.0.1", help="Bind address")
    parser.add_argument(
        "--model",
        default=DEFAULT_MODEL,
        help=f"Model name on HuggingFace Hub (default: {DEFAULT_MODEL})",
    )
    args = parser.parse_args()

    global TTS

    print(f"[kitten_server] Loading model: {args.model}", file=sys.stderr, flush=True)
    TTS = load_model(model_name=args.model)
    print(
        f"[kitten_server] Model loaded. Voices: {AVAILABLE_VOICES}",
        file=sys.stderr,
        flush=True,
    )

    server = HTTPServer((args.host, args.port), KittenHandler)
    print(
        f"[kitten_server] Listening on http://{args.host}:{args.port}",
        file=sys.stderr,
        flush=True,
    )

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[kitten_server] Shutting down", file=sys.stderr, flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
