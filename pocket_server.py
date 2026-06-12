#!/usr/bin/env python3
"""
HTTP server for Pocket TTS — keeps the Python interpreter warm.

NOTE: pocket-tts is a Rust CLI binary with no Python import API. Model
loading happens per-request via subprocess to the CLI. The benefit of this
server over direct Rust-side CLI spawning is:
  - No temp-file dance on the Rust side
  - Consistent API across all local engines
  - Single place for error handling / future optimisations

Usage:
    python pocket_server.py --port 51236

Install deps:
    pip install pocket-tts
    (No Python deps needed at runtime — uses subprocess to call the CLI)

Endpoints (mirror Piper HTTP server contract):
    POST /          {"text": "...", "voice": "alba", "length_scale": 1.0}  → WAV bytes
    GET  /voices     → {"voices": [...]}   (also used as health check)
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler

# ---------------------------------------------------------------------------
# Hardcoded voice list (matches CopySpeak frontend: local-engine.svelte)
# ---------------------------------------------------------------------------
POCKET_VOICES = [
    "alba",
    "marius",
    "javert",
    "jean",
    "fantine",
    "cosette",
    "eponine",
    "azelma",
]

DEFAULT_VOICE = "alba"

# ---------------------------------------------------------------------------
# Globals
# ---------------------------------------------------------------------------
AVAILABLE_VOICES = POCKET_VOICES[:]


def synthesize_via_cli(text, voice, length_scale):
    """Spawn pocket-tts CLI, collect WAV output."""
    # Pocket CLI doesn't support speed; length_scale ignored for API compat
    _ = length_scale

    fd, out_path = tempfile.mkstemp(suffix=".wav", prefix="pocket_server_")
    os.close(fd)

    try:
        result = subprocess.run(
            [
                "pocket-tts",
                "generate",
                "--voice", voice,
                "--text", text,
                "--output-path", out_path,
            ],
            capture_output=True,
            text=True,
            timeout=120,
        )

        if result.returncode != 0:
            stderr = result.stderr.strip()
            stdout = result.stdout.strip()
            msg = f"pocket-tts exited with code {result.returncode}"
            if stderr:
                msg += f": {stderr}"
            if stdout:
                msg += f"\nstdout: {stdout}"
            raise RuntimeError(msg)

        with open(out_path, "rb") as f:
            return f.read()

    finally:
        try:
            os.unlink(out_path)
        except OSError:
            pass


# ---------------------------------------------------------------------------
# HTTP handler
# ---------------------------------------------------------------------------
class PocketHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[pocket_server] {args[0]}", file=sys.stderr, flush=True)

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
                    f"[pocket_server] Unknown voice '{voice}', falling back to {DEFAULT_VOICE}",
                    file=sys.stderr,
                    flush=True,
                )
                voice = DEFAULT_VOICE

            wav_bytes = synthesize_via_cli(text, voice, length_scale)

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
            print(f"[pocket_server] Synthesis error:\n{tb}", file=sys.stderr, flush=True)
            self.wfile.write(tb.encode())


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="Pocket TTS HTTP server")
    parser.add_argument("--port", type=int, required=True, help="TCP port to listen on")
    parser.add_argument("--host", default="127.0.0.1", help="Bind address")
    args = parser.parse_args()

    # Quick startup check: is pocket-tts on PATH?
    try:
        subprocess.run(
            ["pocket-tts", "--help"],
            capture_output=True,
            timeout=10,
        )
    except FileNotFoundError:
        print(
            "[pocket_server] FATAL: 'pocket-tts' not found on PATH. Install with: pip install pocket-tts",
            file=sys.stderr,
            flush=True,
        )
        sys.exit(1)

    print(
        f"[pocket_server] Ready (CLI-based, model loads per-request). Voices: {AVAILABLE_VOICES}",
        file=sys.stderr,
        flush=True,
    )

    server = HTTPServer((args.host, args.port), PocketHandler)
    print(
        f"[pocket_server] Listening on http://{args.host}:{args.port}",
        file=sys.stderr,
        flush=True,
    )

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[pocket_server] Shutting down", file=sys.stderr, flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
