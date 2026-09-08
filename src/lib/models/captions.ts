export interface WordTiming {
  text_start: number;
  text_end: number;
  start_ms: number;
  end_ms: number;
}

export interface CaptionAlignment {
  text: string;
  words: WordTiming[];
}

export interface CaptionWord {
  text: string;
  offset: number;
  start: number | null;
  end: number | null;
  phrase: number;
}

/** Untrusted IPC/history metadata must never cause an invented highlight. */
export function validCaptionAlignment(
  value: CaptionAlignment | null | undefined
): value is CaptionAlignment {
  if (!value || typeof value.text !== "string" || !Array.isArray(value.words)) return false;
  let textEnd = 0;
  let audioEnd = 0;
  const boundary = (offset: number) => {
    const previous = value.text.charCodeAt(offset - 1);
    const next = value.text.charCodeAt(offset);
    return !(previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff);
  };
  for (const word of value.words) {
    if (
      !word ||
      !Number.isInteger(word.text_start) ||
      !Number.isInteger(word.text_end) ||
      word.text_start < textEnd ||
      word.text_end <= word.text_start ||
      word.text_end > value.text.length ||
      !boundary(word.text_start) ||
      !boundary(word.text_end) ||
      !Number.isFinite(word.start_ms) ||
      !Number.isFinite(word.end_ms) ||
      word.start_ms < audioEnd ||
      word.end_ms < word.start_ms
    )
      return false;
    textEnd = word.text_end;
    audioEnd = word.end_ms;
  }
  return true;
}

/** Segment presentation only. Times are native audio milliseconds, never duration weights. */
export function buildCaptions(text: string, alignment?: CaptionAlignment | null): CaptionWord[] {
  const timings =
    validCaptionAlignment(alignment) && alignment.text === text ? alignment.words : [];
  const words: CaptionWord[] = [];
  let prefix = "";
  let timingIndex = 0;
  for (const segment of new Intl.Segmenter(undefined, { granularity: "word" }).segment(text)) {
    if (segment.isWordLike) {
      const end = segment.index + segment.segment.length;
      while (timingIndex < timings.length && timings[timingIndex].text_end <= segment.index)
        timingIndex++;
      const first = timings[timingIndex];
      let last = first;
      let covered = first && first.text_start <= segment.index ? first.text_end : segment.index;
      for (let i = timingIndex + 1; covered < end && i < timings.length; i++) {
        if (timings[i].text_start > covered) break;
        last = timings[i];
        covered = last.text_end;
      }
      const aligned =
        first &&
        last &&
        first.text_start <= segment.index &&
        covered >= end &&
        last.end_ms > first.start_ms;
      const startMs = aligned ? first.start_ms : null;
      const endMs = aligned ? last.end_ms : null;
      const previous = words.at(-1);
      // A provider may align a normalized group as one unit. Do not split its time.
      if (previous && startMs !== null && previous.start === startMs && previous.end === endMs) {
        previous.text += prefix + segment.segment;
      } else {
        words.push({
          text: prefix + segment.segment,
          offset: segment.index - prefix.length,
          start: startMs,
          end: endMs,
          phrase: 0
        });
      }
      prefix = "";
    } else if (words.length) {
      words[words.length - 1].text += segment.segment;
    } else {
      prefix += segment.segment;
    }
  }
  let phrase = 0;
  let phraseLength = 0;
  let phraseWords = 0;
  for (const word of words) {
    if (phraseWords && (phraseLength + word.text.length > 42 || phraseWords >= 7)) {
      phrase++;
      phraseLength = 0;
      phraseWords = 0;
    }
    word.phrase = phrase;
    phraseLength += word.text.length;
    phraseWords++;
    if (/[.!?。！？][\s"'”’)]*$/.test(word.text)) {
      phrase++;
      phraseLength = 0;
      phraseWords = 0;
    }
  }
  return words;
}

export function activeCaptionWord(words: CaptionWord[], positionMs: number): number {
  if (!Number.isFinite(positionMs) || positionMs < 0) return -1;
  return words.findIndex(
    (word) =>
      word.start !== null && word.end !== null && positionMs >= word.start && positionMs < word.end
  );
}

/** Keep the previous phrase through native silence or an audio underrun. */
export function captionPhrase(words: CaptionWord[], positionMs: number): number {
  let phrase = 0;
  for (const word of words) {
    if (word.start !== null && word.start <= positionMs) phrase = word.phrase;
  }
  return phrase;
}
