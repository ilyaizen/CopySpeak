#!/usr/bin/env python3
"""Stable CopySpeak protocol-v2 wrapper for Qwen3-TTS CustomVoice."""

import argparse
import contextlib
import glob
import json
import os
import re
import sys
import wave
from pathlib import Path

_SENTENCE_END = re.compile(r'[.!?。！？]+["\'\u201d\u2019)]*\s*')
_WORD = re.compile(r"\S+")


def probe_cuda_kernels(torch) -> None:
    """Fail at load, loudly, when this torch build cannot run on this GPU.

    torch.cuda.is_available() only proves a driver and runtime; a build without
    kernels for the card's compute capability (a Blackwell sm_120 card on a
    cu126 build) passes it and dies on the first real inference instead.
    """
    try:
        (torch.zeros(1, device="cuda:0") + 1).cpu()
    except Exception as exc:
        raise RuntimeError(
            f"CUDA is present but this torch build cannot run on this GPU ({exc}); "
            "use a CPU profile (--device cpu)"
        ) from exc


def enable_cuda_dlls() -> None:
    if os.name != "nt":
        return
    try:
        import nvidia
    except ImportError:
        return
    dirs = []
    for pattern in ("*/bin", "*/bin/*"):
        for path in glob.glob(os.path.join(list(nvidia.__path__)[0], pattern)):
            if os.path.isdir(path):
                os.add_dll_directory(path)
                dirs.append(path)
    if dirs:
        os.environ["PATH"] = ";".join(dirs) + ";" + os.environ["PATH"]


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    raise ValueError("provide --text or --text-file")


def utf16_len(text: str) -> int:
    return len(text.encode("utf-16-le")) // 2


def split_sentences(text: str) -> list[str]:
    parts, start = [], 0
    for match in _SENTENCE_END.finditer(text):
        parts.append(text[start : match.end()])
        start = match.end()
    tail = text[start:]
    if tail:
        if parts and not tail.strip():
            parts[-1] += tail
        else:
            parts.append(tail)
    return parts


def word_timings(text: str, duration_ms: float, text_offset: int, ms_offset: float) -> list[dict]:
    """Map exact UTF-16 word offsets onto estimated proportional timings."""
    matches = list(_WORD.finditer(text))
    if not matches:
        return []
    weights = [match.end() - match.start() for match in matches]
    total = sum(weights)
    result, cursor, units, walked = [], 0.0, 0, 0
    for index, (match, width) in enumerate(zip(matches, weights)):
        units += utf16_len(text[walked : match.start()])
        end = duration_ms if index == len(matches) - 1 else cursor + duration_ms * width / total
        result.append({
            "text_start": text_offset + units,
            "text_end": text_offset + units + utf16_len(match.group()),
            "start_ms": round(ms_offset + cursor, 1),
            "end_ms": round(ms_offset + end, 1),
        })
        units += utf16_len(match.group())
        walked = match.end()
        cursor = end
    return result


def pcm16(samples) -> bytes:
    import numpy as np

    clipped = np.clip(np.asarray(samples, dtype=np.float32), -1.0, 1.0)
    return (clipped * 32767.0).astype("<i2").tobytes()


def load_engine(model_name: str, device: str):
    import sys

    import torch

    target = "cuda:0" if device == "cuda" else "cpu"
    dtype = torch.bfloat16 if device == "cuda" else torch.float32
    if device == "cuda":
        # Before from_pretrained: its dtype cast can itself launch a kernel.
        probe_cuda_kernels(torch)
    # ponytail: everything between here and from_pretrained() returning must stay
    # off real stdout — in --serve mode stdout IS the protocol v2 pipe, and a
    # stray banner (qwen_tts's flash-attn warning) ahead of READY 2 kills the
    # daemon handshake (Rust read_line sees a blank line and gives up).
    with contextlib.redirect_stdout(sys.stderr):
        from qwen_tts import Qwen3TTSModel

        return Qwen3TTSModel.from_pretrained(model_name, device_map=target, dtype=dtype)


def stream(tts, voice: str, text: str):
    sentences = split_sentences(text)
    if not any(sentence.strip() for sentence in sentences):
        raise ValueError("no sentences to synthesize")
    joined, words, offset_ms = "", [], 0.0
    for sentence in sentences:
        if not sentence.strip():
            continue
        with contextlib.redirect_stdout(sys.stderr):
            wavs, rate = tts.generate_custom_voice(
                text=sentence.strip(), language="Auto", speaker=voice
            )
        pcm = pcm16(wavs[0])
        if not pcm:
            raise ValueError(f"Qwen3-TTS produced no audio for {sentence.strip()!r}")
        duration_ms = len(pcm) / 2 * 1000 / rate
        words.extend(word_timings(sentence, duration_ms, utf16_len(joined), offset_ms))
        joined += sentence
        offset_ms += duration_ms
        yield pcm, rate, 1, {"text": joined, "words": list(words)}


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wav:
        wav.setnchannels(channels)
        wav.setsampwidth(2)
        wav.setframerate(rate)
        wav.writeframes(pcm)


def serve(tts, voice: str) -> int:
    out = sys.stdout.buffer

    def frame(value) -> None:
        out.write((json.dumps(value) + "\n").encode("utf-8"))

    out.write(b"READY 2\n")
    out.flush()
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            header = False
            for pcm, rate, channels, captions in stream(tts, voice, json.loads(line)["text"]):
                if not header:
                    frame({"ok": True, "sample_rate": rate, "channels": channels, "bits_per_sample": 16})
                    header = True
                frame({"captions": captions})
                frame({"chunk": len(pcm)})
                out.write(pcm)
            if not header:
                raise RuntimeError("no audio produced")
            frame({"end": True})
        except Exception as exc:
            frame({"ok": False, "error": str(exc)})
        out.flush()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Qwen3-TTS CLI wrapper for CopySpeak")
    parser.add_argument("--text")
    parser.add_argument("--text-file")
    parser.add_argument("--voice", default="Aiden")
    parser.add_argument("--output")
    parser.add_argument("--model", default="Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice")
    parser.add_argument("--device", choices=["cpu", "cuda"], default="cpu")
    parser.add_argument("--serve", action="store_true")
    args = parser.parse_args()
    if not args.serve and not args.output:
        print("ERROR: --output is required unless --serve is given", file=sys.stderr)
        return 2
    if args.device == "cuda":
        enable_cuda_dlls()
    try:
        tts = load_engine(args.model, args.device)
        print(f"model: {args.model}; device: {args.device}", file=sys.stderr, flush=True)
        if args.serve:
            return serve(tts, args.voice)
        parts, captions, rate = [], None, 0
        for pcm, rate, _, captions in stream(tts, args.voice, read_text(args)):
            parts.append(pcm)
        write_wav(args.output, b"".join(parts), rate, 1)
        if captions and captions["words"]:
            Path(args.output + ".captions.json").write_text(json.dumps(captions), encoding="utf-8")
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        command = "./scripts/install-qwen.sh --force --smoke-test"
        if sys.platform != "linux":
            command = "./scripts/install-qwen.ps1 -Force" + (" -Cuda -SmokeTest" if args.device == "cuda" else "")
        elif args.device == "cuda":
            command = "./scripts/install-qwen.sh --force --cuda --smoke-test"
        print(f"Reinstall with: {command}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
