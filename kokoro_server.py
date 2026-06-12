#!/usr/bin/env python3
"""
HTTP server for Kokoro TTS — keeps the ONNX model loaded in RAM.

Start once, reuse across all synthesis requests. Avoids the cold-start
model-load penalty (~2-8s for the ~500MB ONNX model) on every utterance.

Usage:
    python kokoro_server.py --port 51234
    python kokoro_server.py --port 51234 --model ./kokoro-v1.0.onnx --voices ./voices-v1.0.bin

Install deps:
    pip install kokoro soundfile numpy

Endpoints (mirror Piper HTTP server contract):
    POST /          {"text": "...", "voice": "af_heart", "length_scale": 1.0}  → WAV bytes
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
KOKORO_VOICES = [
    "af_heart",
    "af_bella",
    "af_nicole",
    "af_sarah",
    "af_sky",
    "am_adam",
    "am_michael",
    "bf_emma",
    "bf_isabella",
    "bm_george",
    "bm_lewis",
]

# ---------------------------------------------------------------------------
# Globals set at startup
# ---------------------------------------------------------------------------
PIPELINE = None
AVAILABLE_VOICES = KOKORO_VOICES[:]
DEFAULT_VOICE = "af_heart"


def load_model(model_path=None, voices_path=None):
    """Load Kokoro ONNX model once. Returns KPipeline instance."""
    from kokoro import KPipeline

    pipeline = KPipeline(
        lang_code="a",
        model=model_path,
        voices=voices_path,
    )
    return pipeline


def synthesize(text, voice, length_scale):
    """Run inference, return WAV bytes."""
    import numpy as np
    import soundfile as sf

    # Map Piper's length_scale to speed: speed = 1.0 / length_scale
    speed = 1.0 / max(length_scale, 0.1)

    audio, sample_rate = PIPELINE(text, voice=voice, speed=speed)

    # Kokoro returns float32 numpy array; convert to WAV bytes
    buf = io.BytesIO()
    sf.write(buf, audio, sample_rate, format="WAV")
    return buf.getvalue()


# ---------------------------------------------------------------------------
# HTTP handler
# ---------------------------------------------------------------------------
class KokoroHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        """Log to stderr so Rust can tail it for diagnostics."""
        print(f"[kokoro_server] {args[0]}", file=sys.stderr, flush=True)

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

            # Validate voice
            if voice not in AVAILABLE_VOICES:
                print(
                    f"[kokoro_server] Unknown voice '{voice}', falling back to {DEFAULT_VOICE}",
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
            print(f"[kokoro_server] Synthesis error:\n{tb}", file=sys.stderr, flush=True)
            self.wfile.write(tb.encode())


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="Kokoro TTS HTTP server")
    parser.add_argument("--port", type=int, required=True, help="TCP port to listen on")
    parser.add_argument("--host", default="127.0.0.1", help="Bind address")
    parser.add_argument("--model", default=None, help="Path to kokoro ONNX model file")
    parser.add_argument("--voices", default=None, help="Path to voices-v1.0.bin")
    args = parser.parse_args()

    global PIPELINE

    print(f"[kokoro_server] Loading Kokoro model...", file=sys.stderr, flush=True)
    PIPELINE = load_model(model_path=args.model, voices_path=args.voices)
    print(f"[kokoro_server] Model loaded. Voices: {AVAILABLE_VOICES}", file=sys.stderr, flush=True)

    server = HTTPServer((args.host, args.port), KokoroHandler)
    print(
        f"[kokoro_server] Listening on http://{args.host}:{args.port}",
        file=sys.stderr,
        flush=True,
    )

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[kokoro_server] Shutting down", file=sys.stderr, flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
