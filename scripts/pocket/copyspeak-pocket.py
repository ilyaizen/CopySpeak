#!/usr/bin/env python3
"""CLI wrapper for Pocket TTS (kyutai-labs/pocket-tts) - used by CopySpeak.

Replaces the third-party `pocket-tts generate` CLI: that path reloads the model
and re-derives the voice state on every invocation, and `load_model()` plus
`get_state_for_audio_prompt()` are the slow parts. Keeping both resident is the
whole point of the daemon.

Reads text (inline or from a file), synthesizes with pocket-tts, and writes a WAV
file at the model's own sample rate.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/pocket python {engine_dir}/pocket/scripts/copyspeak-pocket.py \
        --text-file {input} --voice {voice} --output {output} [--device cuda]

With --serve the model is loaded once and stays in RAM, speaking protocol v2
(see src-tauri/src/tts/local_daemon.rs): READY 2, then one JSON request per
stdin line answered with a format header, length-prefixed 16-bit LE PCM chunks,
and an end frame. Stdin EOF ends the process.
"""

import argparse
import glob
import json
import os
import sys
import wave
from pathlib import Path


def enable_cuda_dlls() -> None:
    """Windows: register the nvidia-* wheel DLL directories.

    onnxruntime-gpu and torch do not locate cuDNN/cuBLAS on their own, and since
    Python 3.8 the process PATH is ignored for extension-module dependencies —
    only os.add_dll_directory counts. Globs both the CUDA 12 wheel layout
    (nvidia/<pkg>/bin) and CUDA 13, which consolidates under nvidia/cu13/bin/<arch>.

    A no-op off Windows and when the nvidia-* wheels are not installed; the
    caller then fails loudly at session creation rather than silently on CPU.
    """
    if os.name != "nt":
        return
    try:
        import nvidia
    except ImportError:
        print(
            "WARNING: --device cuda but no nvidia-* runtime wheels in this project; "
            "re-run the installer with -Cuda",
            file=sys.stderr,
            flush=True,
        )
        return
    root = list(nvidia.__path__)[0]
    for pattern in ("*/bin", "*/bin/*"):
        for path in glob.glob(os.path.join(root, pattern)):
            if os.path.isdir(path):
                os.add_dll_directory(path)


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    print("ERROR: provide --text or --text-file", file=sys.stderr)
    sys.exit(2)


def pcm16(samples) -> bytes:
    """float32 in [-1, 1] -> signed 16-bit little-endian PCM bytes."""
    import numpy as np

    clipped = np.clip(np.asarray(samples, dtype=np.float32), -1.0, 1.0)
    return (clipped * 32767.0).astype("<i2").tobytes()


def stream(model, voice_state, text: str):
    """Yield (pcm16_le_bytes, sample_rate, channels)."""
    audio = model.generate_audio(voice_state, text)
    # generate_audio returns a tensor on the model's device, so a CUDA run has
    # to come back to the host before numpy can see it.
    yield pcm16(audio.detach().cpu().numpy()), model.sample_rate, 1


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        wf.setnchannels(channels)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        wf.writeframes(pcm)


def serve(model, voice_state) -> int:
    """Daemon mode, protocol v2. Model stays resident; PCM is framed on stdout.

    Everything goes through sys.stdout.buffer — mixing text and binary writes to
    the same fd would let their separate buffers reorder the framing.

    Synthesis is warmed before the handshake: the first call after a model load
    pays one-off costs (phonemizer init, ONNX graph warmup) that later calls do
    not, and folding that into startup keeps the first real utterance as fast as
    every one after it. CopySpeak prewarms in the background, so this is free.
    """
    out = sys.stdout.buffer

    def frame(obj) -> None:
        out.write((json.dumps(obj) + "\n").encode("utf-8"))

    # Best effort: a warmup failure is not a reason to refuse to serve, and the
    # real request will surface the same error properly framed.
    try:
        for _ in stream(model, voice_state, "Ready."):
            pass
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"WARNING: warmup failed: {exc}", file=sys.stderr, flush=True)

    out.write(b"READY 2\n")
    out.flush()

    while True:
        line = sys.stdin.readline()
        if not line:
            return 0
        line = line.strip()
        if not line:
            continue
        try:
            chunks = stream(model, voice_state, json.loads(line)["text"])
            first = next(chunks, None)
            if first is None:
                raise RuntimeError("no audio produced")
            pcm, rate, channels = first
            frame({
                "ok": True,
                "sample_rate": rate,
                "channels": channels,
                "bits_per_sample": 16,
            })
            while True:
                frame({"chunk": len(pcm)})
                out.write(pcm)
                nxt = next(chunks, None)
                if nxt is None:
                    break
                pcm = nxt[0]
            frame({"end": True})
        except Exception as exc:  # pragma: no cover - environment dependent
            frame({"ok": False, "error": str(exc)})
        out.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description="Pocket TTS CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument("--voice", default="alba", help="Pocket voice name, or a path to a wav/safetensors prompt")
    parser.add_argument("--output", help="Output WAV file path (required unless --serve)")
    parser.add_argument("--serve", action="store_true", help="Keep the model in RAM and read JSON requests on stdin")
    parser.add_argument("--device", default="cpu", choices=["cpu", "cuda"], help="Inference device")
    args = parser.parse_args()

    if args.serve:
        text = None
    else:
        if not args.output:
            print("ERROR: --output is required unless --serve is given", file=sys.stderr)
            return 2
        text = read_text(args)

    if args.device == "cuda":
        enable_cuda_dlls()

    try:
        from pocket_tts import TTSModel
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-pocket.ps1 -Force", file=sys.stderr)
        return 1

    try:
        model = TTSModel.load_model()
        if args.device == "cuda":
            import torch

            if not torch.cuda.is_available():
                # torch reports this as a warning and falls back silently, which
                # would look like "CUDA is on but nothing got faster".
                raise RuntimeError(
                    "torch.cuda.is_available() is False — install a torch build "
                    "matching your driver's CUDA version (install-pocket.ps1 -Cuda)"
                )
            # load_model() has no device argument; TTSModel is a plain nn.Module,
            # so moving it is the documented way to run on the GPU.
            model.to("cuda")
        # Deriving the voice state is slow, so it is done once and reused for
        # every request this daemon serves.
        voice_state = model.get_state_for_audio_prompt(args.voice)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks whether the GPU is actually in use.
    print(f"device: {args.device}", file=sys.stderr, flush=True)

    if args.serve:
        return serve(model, voice_state)

    try:
        pcm, rate, channels = next(stream(model, voice_state, text))
        write_wav(args.output, pcm, rate, channels)
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
