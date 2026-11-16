"""Edge audio and native word boundaries from the same request (no SRT rounding)."""
import asyncio
import json
import sys
import re
import math
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from pathlib import Path


def retry_after_seconds(value, now=None):
    if not isinstance(value, str):
        return None
    value = value.strip()
    if value.isascii() and value.isdigit():
        try:
            return min(int(value), 2**64 - 1)
        except ValueError:
            return None
    try:
        deadline = parsedate_to_datetime(value)
        if deadline.tzinfo is None:
            return None
        return max(0, math.ceil((deadline - (now or datetime.now(timezone.utc))).total_seconds()))
    except (ValueError, TypeError, OverflowError):
        return None


def report_error(error):
    status = getattr(error, "status", None)
    payload = {"kind": "failure"}
    if isinstance(status, int) and not isinstance(status, bool) and 400 <= status <= 599:
        payload = {"kind": "http", "status": status}
        headers = getattr(error, "headers", None)
        delay = retry_after_seconds(headers.get("Retry-After") if headers else None)
        if delay is not None:
            payload["retry_after_seconds"] = delay
    elif isinstance(error, TimeoutError):
        payload = {"kind": "timeout"}
    elif type(error).__name__ == "NoAudioReceived":
        payload = {"kind": "no_audio"}
    elif is_transport_error(error):
        payload = {"kind": "connection"}
    # The leading newline keeps the marker on its own line even if a client or
    # uv left a partial line (no trailing newline) just before exiting; Rust
    # matches the prefix at the start of a line.
    print("\n[edge-error] " + json.dumps(payload), file=sys.stderr, flush=True)


def is_transport_error(error):
    """Whether the failure came from the socket rather than the service.

    Class names are matched by name (not import) so this stays valid whichever
    aiohttp version edge-tts pulls in. Connector subclasses inherit from
    `ClientConnectionError`, including DNS failures.
    """
    for klass in type(error).__mro__:
        name = klass.__name__
        if name.endswith("ConnectionError") or name.startswith("ServerDisconnected"):
            return True
    return False


def preserve_source_text(text, spoken, words):
    """Keep original punctuation when every returned word maps exactly in order.

    If normalization changed the words, display the provider's spoken text;
    never pretend its offsets refer to the original spelling.
    """
    cursor = 0
    mapped = []
    encoded = spoken.encode("utf-16-le")
    for word in words:
        token = encoded[word["text_start"] * 2:word["text_end"] * 2].decode("utf-16-le")
        match = re.search(re.escape(token), text[cursor:], re.IGNORECASE)
        if match is None or any(c.isalnum() for c in text[cursor:cursor + match.start()]):
            return {"text": spoken, "words": words}
        start, end = cursor + match.start(), cursor + match.end()
        mapped.append({**word, "text_start": len(text[:start].encode("utf-16-le")) // 2,
                       "text_end": len(text[:end].encode("utf-16-le")) // 2})
        cursor = end
    if any(c.isalnum() for c in text[cursor:]):
        return {"text": spoken, "words": words}
    return {"text": text, "words": mapped}


async def synthesize(input_path, output_path, voice):
    import edge_tts

    text = Path(input_path).read_text(encoding="utf-8")

    async def speak():
        """One attempt against the endpoint the installed client selects.

        Returns the audio bytes and word boundaries so a failed attempt's
        partial output is discarded with everything else.
        """
        words = []
        spoken = ""
        audio = bytearray()
        # The CLI defaults to SentenceBoundary; explicitly request individual words.
        communicate = edge_tts.Communicate(text, voice, boundary="WordBoundary")
        async for chunk in communicate.stream():
            if chunk["type"] == "audio":
                audio.extend(chunk["data"])
            elif chunk["type"] == "WordBoundary":
                if spoken:
                    spoken += " "
                start = len(spoken.encode("utf-16-le")) // 2
                spoken += chunk["text"]
                words.append({
                    "text_start": start,
                    "text_end": len(spoken.encode("utf-16-le")) // 2,
                    "start_ms": chunk["offset"] / 10000,
                    "end_ms": (chunk["offset"] + chunk["duration"]) / 10000,
                })
        return bytes(audio), spoken, words

    audio, spoken, words = await speak()

    with open(output_path, "wb") as out:
        out.write(audio)
    Path(output_path + ".captions.json").write_text(
        json.dumps(preserve_source_text(text, spoken, words)), encoding="utf-8"
    )


def main(argv):
    """Run one synthesis; return the process exit code.

    Split out of the `__main__` guard so the error path is testable: it is the
    only place that reports structured failure metadata to the Rust caller.
    """
    try:
        asyncio.run(synthesize(*argv))
    except Exception as error:
        report_error(error)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
