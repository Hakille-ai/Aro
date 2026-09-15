const COMBINING_MARKS = /[\u0300-\u036f]/g;
const PUNCTUATION = /[.,/#!$%^&*;:{}=\-_`~()?"'\[\]<>|\\]+/g;

export function normalizeVoiceText(text: string): string {
  return text
    .normalize("NFD")
    .replace(COMBINING_MARKS, "")
    .toLowerCase()
    .replace(PUNCTUATION, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export function voiceWords(text: string): string[] {
  const normalized = normalizeVoiceText(text);
  return normalized ? normalized.split(" ") : [];
}
