#!/usr/bin/env python3
"""CLI wrapper for tiny-voca (English, 4.5M parameters) - used by CopySpeech.

Reads text (inline or from a file), synthesizes with the tiny-voca ONNX model,
and writes a 24kHz mono WAV file. Kept stable so CopySpeech's args_template
never changes when upstream shifts.

Invoked by CopySpeech via:
    uv run --project {engine_dir}/tiny-voca python {engine_dir}/tiny-voca/scripts/copyspeech-tiny-voca.py \\
        --text-file {input} --voice {voice} --output {output} [--speed 1.0]

With --serve the model stays in RAM and the wrapper speaks protocol v2
(see src-tauri/src/tts/local_daemon.rs): READY 2, then one JSON request per
stdin line answered with a format header, caption-frame-plus-PCM-chunk pairs,
and an end frame. Stdin EOF ends the process.

The model reads one utterance per call (up to 512 tokens, 32 s), so the text is
synthesized one sentence at a time: each sentence becomes one PCM chunk, so
first audio flows without waiting for the whole request. Each chunk is preceded
by a cumulative caption frame. Word times are estimated proportionally from each
sentence's real audio duration; the model exposes no alignment.

Text is converted to phonemes with dizione (English only). A sentence with
nothing to pronounce is skipped. If no sentence can be read, the request fails
with an error saying so; the wrapper never returns silent audio.

Weights live in <engine_dir>/weights (fetched by install-tiny-voca.ps1):
    weights/onnx/model.onnx   the model (ONNX Runtime, CPU)
    weights/phonemizer/       dizione data, read through DIZIONE_DATA
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
VOICE_IDS = {"F2": 0, "M2": 1}
DEFAULT_VOICE = "M2"
DEFAULT_WEIGHTS = Path(__file__).resolve().parents[1] / "weights"

# Sentence end: terminal punctuation plus any closing quotes and the trailing
# whitespace. The pieces concatenate back to the input text.
_SENTENCE_END = re.compile(r"[.!?。！？]+[\"'”’)]*\s*")
_TERMINAL = (".", "!", "?", "。", "！", "？")
_WORD = re.compile(r"\S+")


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
    weight - an honest estimate, since the model exposes no alignment. The last
    word ends exactly at the audio's end. Values stay unrounded: rounding happens
    once, in offset_timings.
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
    """Return (session, phonemize) for the ONNX model and the dizione frontend.

    DIZIONE_DATA must point at the phonemizer data before dizione is imported.
    The model's graph takes phoneme ids, so the text-to-ids step runs in Python.
    """
    model_path = weights_dir / "onnx" / "model.onnx"
    if not model_path.is_file():
        raise RuntimeError(f"model not found: {model_path}; re-run install-tiny-voca.ps1")
    phonemizer_dir = weights_dir / "phonemizer"
    if not phonemizer_dir.is_dir():
        raise RuntimeError(f"phonemizer data not found: {phonemizer_dir}; re-run install-tiny-voca.ps1")
    os.environ["DIZIONE_DATA"] = str(phonemizer_dir)

    # Model load and any library output must never precede the daemon
    # handshake on stdout.
    with contextlib.redirect_stdout(sys.stderr):
        import onnxruntime as ort
        import dizione

        session = ort.InferenceSession(str(model_path), providers=["CPUExecutionProvider"])

    def phonemize(sentence: str):
        """Return the phonemes for one sentence, or None when nothing is spoken.

        dizione (pinned commit) raises ValueError "the text has nothing to
        pronounce" for a sentence of only marks or other scripts. Any other
        ValueError is a real failure and propagates.
        """
        try:
            with contextlib.redirect_stdout(sys.stderr):
                return dizione.phonemize(sentence)
        except ValueError as exc:
            if "nothing to pronounce" in str(exc):
                return None
            raise

    return session, phonemize


def synthesize(session, phones, voice_id: int, speed: float):
    """Run the graph on one sentence's phonemes; return float32 samples in [-1, 1]."""
    import numpy as np

    feeds = {
        "tokens": np.array([phones.ids], dtype=np.int64),
        "stress": np.array([phones.stress], dtype=np.int64),
        "voice": np.array([voice_id], dtype=np.int64),
        "speed": np.array([speed], dtype=np.float32),
        "pitch_shift": np.array([0.0], dtype=np.float32),
        "pitch_range": np.array([1.0], dtype=np.float32),
    }
    audio = session.run(None, feeds)[0][0]
    # The model can peak above 1.0; scale down only then, so no sample clips.
    peak = float(np.abs(audio).max()) if audio.size else 0.0
    if peak > 0.99:
        audio = audio * (0.99 / peak)
    return audio


def stream(session, phonemize, voice_id: int, speed: float, text: str):
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
        body = sentence.strip()
        if not body:
            joined += sentence
            continue
        # The model needs an end mark, or the last word breaks off. The mark is
        # added to the synthesized text only; captions keep the request text.
        spoken_text = body if body.endswith(_TERMINAL) else body + "."
        phones = phonemize(spoken_text)
        if phones is None:
            joined += sentence
            continue
        pcm = pcm16(synthesize(session, phones, voice_id, speed))
        if not pcm:
            raise ValueError(f"tiny-voca produced no audio for {body!r}")
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
            "nothing in the text could be read: tiny-voca only reads English for now"
        )


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        wf.setnchannels(channels)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        wf.writeframes(pcm)


def serve(session, phonemize, voice_id: int, speed: float) -> int:
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
        for _ in stream(session, phonemize, voice_id, speed, "Ready."):
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
            for pcm, rate, channels, captions in stream(
                session, phonemize, voice_id, speed, json.loads(line)["text"]
            ):
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
    parser = argparse.ArgumentParser(description="tiny-voca CLI wrapper for CopySpeech")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument(
        "--voice",
        default=DEFAULT_VOICE,
        choices=sorted(VOICE_IDS),
        help="Voice: F2 (female) or M2 (male)",
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
        session, phonemize = load_engine(args.weights_dir)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks which execution provider runs.
    print(f"providers: {session.get_providers()}", file=sys.stderr, flush=True)
    voice_id = VOICE_IDS[args.voice]

    if args.serve:
        return serve(session, phonemize, voice_id, args.speed)

    try:
        parts: list[bytes] = []
        captions: dict | None = None
        for pcm, rate, channels, captions in stream(session, phonemize, voice_id, args.speed, text):
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
