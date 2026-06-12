#!/usr/bin/env python3
"""
HTTP server for Kokoro TTS — keeps the ONNX model loaded in RAM.

Two modes:
  A) Direct Python API  — `from kokoro_tts import Kokoro` (installed with kokoro-tts)
  B) Subprocess fallback — spawns `kokoro-tts` CLI per request

Mode A keeps the ~500MB ONNX model loaded for sub-second synthesis.
Mode B reloads from disk every request.

Usage:
    python kokoro_server.py --port 51234
    python kokoro_server.py --port 51234 --model ./kokoro-v1.0.onnx --voices ./voices-v1.0.bin

Endpoints (mirror Piper HTTP server contract):
    POST /          {"text": "...", "voice": "af_heart", "length_scale": 1.0}  → WAV bytes
    GET  /voices     → {"voices": [...]}   (also used as health check)
"""

import argparse
import io
import json
import os
import subprocess
import sys
import tempfile
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler

KOKORO_VOICES = [
    "af_heart", "af_bella", "af_nicole", "af_sarah", "af_sky",
    "am_adam", "am_michael", "bf_emma", "bf_isabella",
    "bm_george", "bm_lewis",
]

DEFAULT_VOICE = "af_heart"

# ---------------------------------------------------------------------------
# Globals
# ---------------------------------------------------------------------------
ENGINE = None
AVAILABLE_VOICES = KOKORO_VOICES[:]
USE_CLI_FALLBACK = False
CLI_MODEL_PATH = None
CLI_VOICES_PATH = None


def try_load_engine(model_path, voices_path):
    """Try to create a Kokoro engine. Returns None if unavailable."""
    try:
        from kokoro_tts import Kokoro

        engine = Kokoro(
            model_path or "",
            voices_path or "",
        )
        return engine
    except ImportError:
        return None
    except Exception as e:
        print(f"[kokoro_server] Engine load failed: {e}", file=sys.stderr, flush=True)
        return None


def synthesize_direct(text, voice, length_scale):
    """Direct inference — model stays loaded in ENGINE."""
    import numpy as np
    import soundfile as sf

    speed = 1.0 / max(length_scale, 0.1)
    audio, sr = ENGINE.create(text, voice=voice, speed=speed)
    buf = io.BytesIO()
    sf.write(buf, audio, sr, format="WAV")
    return buf.getvalue()


def synthesize_via_cli(text, voice, length_scale):
    """Spawn kokoro-tts CLI, collect WAV output. Model reloads each call."""
    _ = length_scale

    fd_out, out_path = tempfile.mkstemp(suffix=".wav", prefix="kokoro_server_")
    os.close(fd_out)
    fd_in, in_path = tempfile.mkstemp(suffix=".txt", prefix="kokoro_server_")
    os.close(fd_in)

    try:
        with open(in_path, "w", encoding="utf-8") as f:
            f.write(text)

        cmd = [
            "kokoro-tts",
            in_path,
            out_path,
            "--voice", voice,
        ]
        if CLI_MODEL_PATH:
            cmd.extend(["--model", CLI_MODEL_PATH])
        if CLI_VOICES_PATH:
            cmd.extend(["--voices", CLI_VOICES_PATH])

        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)

        if result.returncode != 0:
            stderr = result.stderr.strip()
            stdout = result.stdout.strip()
            msg = f"kokoro-tts exited with code {result.returncode}"
            if stderr:
                msg += f": {stderr}"
            if stdout:
                msg += f"\nstdout: {stdout}"
            raise RuntimeError(msg)

        with open(out_path, "rb") as f:
            return f.read()

    finally:
        for p in (out_path, in_path):
            try:
                os.unlink(p)
            except OSError:
                pass


class KokoroHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
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

            if voice not in AVAILABLE_VOICES:
                print(
                    f"[kokoro_server] Unknown voice '{voice}', falling back to {DEFAULT_VOICE}",
                    file=sys.stderr, flush=True,
                )
                voice = DEFAULT_VOICE

            if USE_CLI_FALLBACK:
                wav_bytes = synthesize_via_cli(text, voice, length_scale)
            else:
                wav_bytes = synthesize_direct(text, voice, length_scale)

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


def main():
    parser = argparse.ArgumentParser(description="Kokoro TTS HTTP server")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--model", default=None, help="Path to kokoro ONNX model file")
    parser.add_argument("--voices", default=None, help="Path to voices-v1.0.bin")
    args = parser.parse_args()

    global ENGINE, USE_CLI_FALLBACK, CLI_MODEL_PATH, CLI_VOICES_PATH

    CLI_MODEL_PATH = args.model
    CLI_VOICES_PATH = args.voices

    print("[kokoro_server] Loading Kokoro model...", file=sys.stderr, flush=True)
    ENGINE = try_load_engine(model_path=args.model, voices_path=args.voices)

    if ENGINE is not None:
        USE_CLI_FALLBACK = False
        print(
            f"[kokoro_server] Model loaded via kokoro_tts API. "
            f"Voices: {AVAILABLE_VOICES}",
            file=sys.stderr, flush=True,
        )
    else:
        USE_CLI_FALLBACK = True
        print(
            "[kokoro_server] kokoro_tts not importable. "
            "Falling back to CLI subprocess (model loads per-request).",
            file=sys.stderr, flush=True,
        )

    server = HTTPServer((args.host, args.port), KokoroHandler)
    print(f"[kokoro_server] Listening on http://{args.host}:{args.port}", file=sys.stderr, flush=True)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[kokoro_server] Shutting down", file=sys.stderr, flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
