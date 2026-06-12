#!/usr/bin/env python3
"""
HTTP server for Pocket TTS — keeps the model loaded in RAM.

Uses the pocket_tts Python API directly (TTSModel.load_model stays resident).
Installed alongside `pocket-tts` CLI.

Usage:
    python pocket_server.py --port 51236
    python pocket_server.py --port 51236 --language english --device cpu

Endpoints (mirror Piper HTTP server contract):
    POST /          {"text": "...", "voice": "alba", "length_scale": 1.0}  → WAV bytes
    GET  /voices     → {"voices": [...]}   (also used as health check)
"""

import argparse
import io
import json
import sys
import traceback
import wave
from http.server import HTTPServer, BaseHTTPRequestHandler

import torch

POCKET_VOICES = [
    "alba", "marius", "javert", "jean",
    "fantine", "cosette", "eponine", "azelma",
]

DEFAULT_VOICE = "alba"

MODEL = None
SAMPLE_RATE = None
AVAILABLE_VOICES = POCKET_VOICES[:]


def load_model(language="english", device="cpu"):
    """Load Pocket TTS model once."""
    from pocket_tts.models.tts_model import TTSModel

    model = TTSModel.load_model(language=language)
    model.to(device)
    return model


def synthesize(text, voice, length_scale):
    """Run inference via direct Python API, return WAV bytes."""
    _ = length_scale  # Pocket doesn't expose speed at synthesis time

    model_state = MODEL.get_state_for_audio_prompt(voice)
    chunks = MODEL.generate_audio_stream(
        model_state=model_state,
        text_to_generate=text,
    )
    wav_tensor = torch.cat(list(chunks), dim=0)

    int16 = (wav_tensor.clamp(-1, 1) * 32767).short().detach().cpu()
    pcm = int16.numpy().tobytes()

    buf = io.BytesIO()
    w = wave.open(buf, "wb")
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SAMPLE_RATE)
    w.writeframes(pcm)
    w.close()
    return buf.getvalue()


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
            print(f"[pocket_server] Synthesis error:\n{tb}", file=sys.stderr, flush=True)
            self.wfile.write(tb.encode())


def main():
    parser = argparse.ArgumentParser(description="Pocket TTS HTTP server")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--language", default="english")
    parser.add_argument("--device", default="cpu")
    args = parser.parse_args()

    global MODEL, SAMPLE_RATE

    print("[pocket_server] Loading Pocket TTS model...", file=sys.stderr, flush=True)
    MODEL = load_model(language=args.language, device=args.device)
    SAMPLE_RATE = MODEL.config.mimi.sample_rate
    print(
        f"[pocket_server] Model loaded. Voices: {AVAILABLE_VOICES}, sr={SAMPLE_RATE}Hz",
        file=sys.stderr,
        flush=True,
    )

    server = HTTPServer((args.host, args.port), PocketHandler)
    print(f"[pocket_server] Listening on http://{args.host}:{args.port}", file=sys.stderr, flush=True)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("[pocket_server] Shutting down", file=sys.stderr, flush=True)
        server.server_close()


if __name__ == "__main__":
    main()
