export interface CaptionWord {
  text: string;
  start: number;
  end: number;
  phrase: number;
}

/** Relative word timings, scaled to the fragment's audio duration by the player. */
export function buildCaptions(text: string): CaptionWord[] {
  const words: CaptionWord[] = [];
  let prefix = "";
  for (const segment of new Intl.Segmenter(undefined, { granularity: "word" }).segment(text)) {
    if (segment.isWordLike) {
      words.push({ text: prefix + segment.segment, start: 0, end: 0, phrase: 0 });
      prefix = "";
    } else if (words.length) {
      words[words.length - 1].text += segment.segment;
    } else {
      prefix += segment.segment;
    }
  }
  let elapsed = 0;
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
    word.start = elapsed;
    // ponytail: estimated pronunciation; replace weights with engine word boundaries
    // or forced alignment if listening tests require precise synchronization.
    const letters = Array.from(word.text.match(/[\p{L}\p{N}]/gu) ?? []).length;
    elapsed +=
      0.6 +
      Math.sqrt(letters) +
      (/[.!?。！？][\s"'”’)]*$/.test(word.text)
        ? 1.5
        : /[,;:،][\s"'”’)]*$/.test(word.text)
          ? 0.7
          : 0);
    word.end = elapsed;
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

export function activeCaptionWord(
  words: CaptionWord[],
  positionMs: number,
  durationMs: number
): number {
  if (
    !words.length ||
    !Number.isFinite(positionMs) ||
    positionMs < 0 ||
    !Number.isFinite(durationMs) ||
    durationMs <= 0
  )
    return -1;
  const position = Math.min(1, positionMs / durationMs) * words[words.length - 1].end;
  const index = words.findIndex((word) => position < word.end);
  return index === -1 ? words.length - 1 : index;
}

export function estimateCaptionDuration(words: CaptionWord[]): number {
  return (words.at(-1)?.end ?? 0) * 130;
}
