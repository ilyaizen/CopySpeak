const MAX_DISPLAY_LENGTH = 1000;
const MAX_VOICE_LENGTH = 20;
const ELLIPSIS = "\u2026";

export function formatTextForDisplay(text: string): string {
  return text
    .replace(/([^\.\!\?\;\:\,])\n/g, "$1. ")
    .replace(/\n/g, " ")
    .trim()
    .substring(0, MAX_DISPLAY_LENGTH);
}

export function formatVoiceLabel(voice: string): string {
  if (voice.length > MAX_VOICE_LENGTH) {
    return voice.substring(0, MAX_VOICE_LENGTH) + ELLIPSIS;
  }
  return voice;
}

export function formatProviderVoiceLabel(
  provider: string | null,
  voice: string | null
): string | null {
  if (!provider && !voice) return null;
  const p = provider ?? "";
  const v = voice ? formatVoiceLabel(voice) : "";
  if (p && v) return `${p} \u00b7 ${v}`;
  return p || v;
}
