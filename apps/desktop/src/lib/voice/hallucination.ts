import { normalizeVoiceText } from "./text";

const BRACKETED_MARKER = /^\s*(\[.*\]|\(.*\)|\*.*\*)\s*$/;
// Notes de musique, points de suspension et symboles isolés : Whisper les
// émet seuls sur du silence/bruit (ex. "♪", "…"). Ils ne sont ni ponctuation
// ASCII ni lettres, donc testés explicitement avant normalisation.
const SYMBOL_ONLY = /^[♪♫…\u2000-\u206F\u2600-\u27BF\s]+$/;
const PUNCTUATION_ONLY = /^[.,/#!$%^&*;:{}=\-_`~()?"'\s]+$/;

const DEFAULT_WHISPER_HALLUCINATIONS = [
  "amara org",
  "blank",
  "bruit",
  "fin de la musique",
  "generique",
  "generique de fin",
  "laughter",
  "merci",
  "merci beaucoup",
  "music",
  "musique",
  "noise",
  "rires",
  "silence",
  "sourire",
  "sous titrage societe radio canada",
  "sous titres",
  "sous titres par",
  "thank you",
  "thank you for watching",
  "thank you very much",
  "thanks for watching",
] as const;

export interface WhisperHallucinationOptions {
  phrases?: readonly string[];
}

export function isWhisperHallucination(
  text: string,
  options: WhisperHallucinationOptions = {},
): boolean {
  const trimmed = text.trim();
  if (!trimmed) {
    return true;
  }

  if (
    BRACKETED_MARKER.test(trimmed) ||
    SYMBOL_ONLY.test(trimmed) ||
    PUNCTUATION_ONLY.test(trimmed)
  ) {
    return true;
  }

  const clean = normalizeVoiceText(trimmed);
  if (!clean) {
    return true;
  }

  const phrases = new Set(
    (options.phrases ?? DEFAULT_WHISPER_HALLUCINATIONS).map((phrase) => normalizeVoiceText(phrase)),
  );

  if (phrases.has(clean)) {
    return true;
  }

  return clean.startsWith("sous titres par ");
}
