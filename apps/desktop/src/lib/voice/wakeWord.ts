import { normalizeVoiceText, voiceWords } from "./text";

export interface WakeWordMatchOptions {
  variants?: readonly string[];
  allowEmbeddedAro?: boolean;
}

const DEFAULT_WAKE_WORD_VARIANTS = ["aro", "haro", "arrow", "aero"] as const;
const ARO_LIKE_WORD = /^h?a+r+o+$/;

export function matchesWakeWord(text: string, options: WakeWordMatchOptions = {}): boolean {
  const variants = new Set(
    (options.variants ?? DEFAULT_WAKE_WORD_VARIANTS).map((variant) => normalizeVoiceText(variant)),
  );

  for (const word of voiceWords(text)) {
    if (variants.has(word) || ARO_LIKE_WORD.test(word)) {
      return true;
    }

    if (options.allowEmbeddedAro === true && word.includes("aro")) {
      return true;
    }
  }

  return false;
}
