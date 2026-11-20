#!/usr/bin/env python3
"""CLI wrapper for kale-tts-1-nano (KaleComputer, English, 7.5M parameters).

Reads text (inline or from a file), synthesizes with the int8 ONNX model, and
writes a 24kHz mono WAV file. Kept stable so CopySpeech's args_template never
changes when upstream shifts.

Invoked by CopySpeech via:
    uv run --project {engine_dir}/kale python {engine_dir}/kale/scripts/copyspeech-kale.py \
        --text-file {input} --voice {voice} --output {output} [--speed 1.0]

With --serve the model stays in RAM and the wrapper speaks protocol v2
(see src-tauri/src/tts/local_daemon.rs): READY 2, then one JSON request per
stdin line answered with a format header, caption-frame-plus-PCM-chunk pairs,
and an end frame. Stdin EOF ends the process.

The model reads one sentence per call (up to 510 phoneme tokens), so the text
is synthesized sentence by sentence: each sentence becomes one PCM chunk, so
first audio flows without waiting for the whole request. Each chunk is
preceded by a cumulative caption frame. Word times are estimated
proportionally from each sentence's real audio duration; the graph exposes the
waveform only, no alignment.

Text goes through the release G2P front end (kale_tts.py + kale_frontend.js,
vendored beside this wrapper by the installer) running in-process via QuickJS.
US and British accents both load when the front-end data is present. A
sentence with nothing to pronounce is skipped. If no sentence can be read, the
request fails with an error saying so; the wrapper never returns silent audio.

Weights live in <engine_dir>/weights and the G2P data in <engine_dir>/frontend
(both fetched by install-kale.ps1 / install-kale.sh from
KaleComputer/kale-tts-1-nano, Apache-2.0). The voices were designed from
one-line text descriptions; none is a real person. Their training audio is
Maya-1 model output and must never be presented as a recording of a human
being (Maya-1 use policy).
"""

import argparse
import contextlib
import json
import os
import re
import sys
import wave
from pathlib import Path

SAMPLE_RATE = 24000
DEFAULT_VOICE = "Kenzie"
DEFAULT_WEIGHTS = Path(__file__).resolve().parents[1] / "weights"

# Sentence end: terminal punctuation plus any closing quotes and the trailing
# whitespace. The pieces concatenate back to the input text.
_SENTENCE_END = re.compile(r"[.!?。！？]+[\"'”’)]*\s*")
_TERMINAL = (".", "!", "?", "。", "！", "？")
_WORD = re.compile(r"\S+")


def _force_utf8_mode() -> None:
    """Re-exec with -X utf8 on Windows when UTF-8 mode is off.

    kale_tts reads the G2P front-end data (heavily non-ASCII) with bare
    read_text() calls, and this wrapper reads protocol JSON from stdin. Both
    default to the locale codepage (cp1252) on Windows, which dies with
    "'charmap' codec can't decode byte 0x90..." on real text. UTF-8 mode
    fixes files and stdio together. Check env first so the re-exec happens
    once: uv and the dev shells set PYTHONUTF8=1 already.
    """
    if os.name != "nt" or sys.flags.utf8_mode or os.environ.get("PYTHONUTF8") == "1":
        return
    import subprocess

    os.environ["PYTHONUTF8"] = "1"
    result = subprocess.run(
        [sys.executable, "-X", "utf8", str(Path(__file__).resolve()), *sys.argv[1:]],
    )
    sys.exit(result.returncode)


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    print("ERROR: provide --text or --text-file", file=sys.stderr)
    sys.exit(2)


def utf16_len(s: str) -> int:
    """UTF-16 code units - the offset unit CopySpeech's captions address."""
    return len(s.encode("utf-16-le")) // 2


def split_sentences(text: str) -> list[str]:
    """Split on terminal punctuation; the pieces concatenate back to `text`.

    Keeping each piece's original whitespace is what makes the cumulative
    caption text byte-identical to the request text, so word offsets stay
    absolute.
    """
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


def word_timings(text: str, duration_ms: float) -> list[dict]:
    """Exact UTF-16 word offsets; raw ms proportional to word length.

    The sentence's real audio duration is spread across its words by character
    weight - an honest estimate, since the graph exposes no alignment. The last
    word ends exactly at the audio's end. Values stay unrounded: rounding
    happens once, in offset_timings.
    """
    matches = list(_WORD.finditer(text))
    if not matches:
        return []
    weights = [match.end() - match.start() for match in matches]
    total = sum(weights)
    timings = []
    cursor = 0.0
    units = 0
    walked = 0
    for index, (match, width) in enumerate(zip(matches, weights)):
        units += utf16_len(text[walked : match.start()])
        walked = match.start()
        cursor_end = (
            duration_ms if index == len(matches) - 1 else cursor + duration_ms * width / total
        )
        timings.append(
            {
                "text_start": units,
                "text_end": units + utf16_len(match.group()),
                "start_ms": cursor,
                "end_ms": cursor_end,
            }
        )
        units += utf16_len(match.group())
        walked = match.end()
        cursor = cursor_end
    return timings


def offset_timings(timings: list[dict], text_offset: int, ms_offset: float) -> list[dict]:
    """Shift one sentence's timings into the request's text and audio clock.

    The single rounding point: every emitted millisecond is rounded once, after
    accumulation, so adjacent sentences' word times stay monotone.
    """
    return [
        {
            "text_start": timing["text_start"] + text_offset,
            "text_end": timing["text_end"] + text_offset,
            "start_ms": round(timing["start_ms"] + ms_offset, 1),
            "end_ms": round(timing["end_ms"] + ms_offset, 1),
        }
        for timing in timings
    ]


def pcm16(samples) -> bytes:
    """float32 in [-1, 1] -> signed 16-bit little-endian PCM bytes.

    CopySpeech's player only accepts 16-bit samples, so the conversion belongs
    here rather than in Rust.
    """
    import numpy as np

    clipped = np.clip(np.asarray(samples, dtype=np.float32), -1.0, 1.0)
    return (clipped * 32767.0).astype("<i2").tobytes()


def load_engine(weights_dir: Path):
    """Return the KaleTTS runtime (session, G2P front end, voice tables).

    kale_tts.py and kale_frontend.js sit beside this wrapper; the G2P word
    data lives in <engine_dir>/frontend, which kale_tts finds relative to
    itself. The QuickJS front end needs no Node. Model load and any library
    output must never precede the daemon handshake on stdout.
    """
    model_path = weights_dir / "kale-tts-1-nano.onnx"
    if not model_path.is_file():
        raise RuntimeError(f"model not found: {model_path}; re-run install-kale.ps1")
    frontend_dir = Path(__file__).resolve().parents[1] / "frontend"
    if not (frontend_dir / "words" / "g2p.bin").is_file():
        raise RuntimeError(f"front-end data not found: {frontend_dir}; re-run install-kale.ps1")

    sys.path.insert(0, str(Path(__file__).resolve().parent))
    with contextlib.redirect_stdout(sys.stderr):
        import numpy as np  # noqa: F401 - kept beside the session import
        from kale_tts import KaleTTS

        tts = KaleTTS(str(weights_dir), providers=["CPUExecutionProvider"], graph="int8")
    return tts


def synthesize(tts, voice, speed: float, sentence: str):
    """Run the front end and graph on one sentence; return float32 samples.

    The front end may split a piece further; every sub-sentence's audio is
    concatenated, so one caption unit covers the whole sentence piece.
    """
    import numpy as np

    meta, style, feeds = tts.voice(voice)
    accent = meta.get("accent", "en-us")
    parts = []
    for sub in tts.frontend(sentence, accent):
        ids = tts._ids(sub["ipa"], accent)[:510]
        if not ids:
            continue
        x = np.array([[0, *ids, 0]], dtype=np.int64)
        batch = {
            "input_ids": x,
            "style": style[min(len(ids), len(style)) - 1][None],
            "speed": np.full(1, speed, dtype=np.float32),
            **feeds,
        }
        parts.append(tts.session.run(None, batch)[0][0])
    if not parts:
        return None
    return np.concatenate(parts) if len(parts) > 1 else parts[0]


def stream(tts, voice, speed: float, text: str):
    """Yield (pcm, rate, channels, captions) per sentence; captions cumulative.

    The caption frame before a chunk covers every sentence so far. Word offsets
    are exact in the joined text; word times are estimated from each sentence's
    real audio duration.
    """
    sentences = split_sentences(text)
    if not any(sentence.strip() for sentence in sentences):
        raise ValueError("no text to synthesize")
    joined = ""
    words: list[dict] = []
    offset_ms = 0.0
    spoken = 0
    for sentence in sentences:
        if not sentence.strip():
            joined += sentence
            continue
        audio = synthesize(tts, voice, speed, sentence)
        if audio is None or audio.size == 0:
            joined += sentence
            continue
        pcm = pcm16(audio)
        duration_ms = len(pcm) / 2 * 1000 / SAMPLE_RATE
        words.extend(
            offset_timings(word_timings(sentence, duration_ms), utf16_len(joined), offset_ms)
        )
        joined += sentence
        offset_ms += duration_ms
        spoken += 1
        yield pcm, SAMPLE_RATE, 1, {"text": joined, "words": list(words)}
    if spoken == 0:
        raise ValueError(
            "nothing in the text could be read: kale reads English text only"
        )


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        wf.setnchannels(channels)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        wf.writeframes(pcm)


def serve(tts, voice, speed: float) -> int:
    """Daemon mode, protocol v2. Model stays resident; PCM is framed on stdout.

    Everything goes through sys.stdout.buffer - mixing text and binary writes to
    the same fd would let their separate buffers reorder the framing.
    """
    out = sys.stdout.buffer

    def frame(obj) -> None:
        out.write((json.dumps(obj) + "\n").encode("utf-8"))

    # Best effort: a warmup failure is not a reason to refuse to serve, and the
    # real request will surface the same error properly framed.
    try:
        for _ in stream(tts, voice, speed, "Ready."):
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
            # The ok header must precede every other frame, so it is emitted
            # from the first sentence rather than up front.
            header = False
            for pcm, rate, channels, captions in stream(tts, voice, speed, json.loads(line)["text"]):
                if not header:
                    frame(
                        {
                            "ok": True,
                            "sample_rate": rate,
                            "channels": channels,
                            "bits_per_sample": 16,
                        }
                    )
                    header = True
                frame({"captions": captions})
                if pcm:
                    frame({"chunk": len(pcm)})
                    out.write(pcm)
            if not header:
                raise RuntimeError("no audio produced")
            frame({"end": True})
        except Exception as exc:  # pragma: no cover - environment dependent
            frame({"ok": False, "error": str(exc)})
        out.flush()


def main() -> int:
    _force_utf8_mode()
    # Protocol JSON reaches the daemon as UTF-8; Windows text-mode stdin
    # defaults to the locale codepage, which mangles non-ASCII text.
    if hasattr(sys.stdin, "reconfigure"):
        sys.stdin.reconfigure(encoding="utf-8")

    parser = argparse.ArgumentParser(description="kale-tts-1-nano CLI wrapper for CopySpeech")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument(
        "--voice",
        default=DEFAULT_VOICE,
        help="Voice name from the catalog, e.g. Kenzie (US) or Amy (British)",
    )
    parser.add_argument(
        "--speed",
        type=float,
        default=1.0,
        help="Speaking rate passed to the model (1 = as predicted)",
    )
    parser.add_argument("--output", help="Output WAV file path (required unless --serve)")
    parser.add_argument(
        "--serve", action="store_true", help="Keep the model in RAM and read JSON requests on stdin"
    )
    parser.add_argument(
        "--weights-dir",
        type=Path,
        default=DEFAULT_WEIGHTS,
        help="Weights folder (default: weights/ beside this engine dir)",
    )
    args = parser.parse_args()

    if args.serve:
        text = None
    else:
        if not args.output:
            print("ERROR: --output is required unless --serve is given", file=sys.stderr)
            return 2
        text = read_text(args)

    try:
        tts = load_engine(args.weights_dir)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    try:
        voice = tts.resolve_voice(args.voice)
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks which execution provider runs.
    print(f"providers: {tts.session.get_providers()}", file=sys.stderr, flush=True)

    if args.serve:
        return serve(tts, voice, args.speed)

    try:
        parts: list[bytes] = []
        captions: dict | None = None
        for pcm, rate, channels, captions in stream(tts, voice, args.speed, text):
            if pcm:
                parts.append(pcm)
        write_wav(args.output, b"".join(parts), SAMPLE_RATE, 1)
        if captions and captions["words"]:
            Path(args.output + ".captions.json").write_text(json.dumps(captions), encoding="utf-8")
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
