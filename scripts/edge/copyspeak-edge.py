"""Edge audio and native word boundaries from the same request (no SRT rounding)."""
import asyncio
import json
import sys
import re
from pathlib import Path


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
    words = []
    spoken = ""
    # The CLI defaults to SentenceBoundary; explicitly request individual words.
    communicate = edge_tts.Communicate(text, voice, boundary="WordBoundary")
    with open(output_path, "wb") as audio:
        async for chunk in communicate.stream():
            if chunk["type"] == "audio":
                audio.write(chunk["data"])
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
    Path(output_path + ".captions.json").write_text(
        json.dumps(preserve_source_text(text, spoken, words)), encoding="utf-8"
    )


if __name__ == "__main__":
    asyncio.run(synthesize(*sys.argv[1:]))
