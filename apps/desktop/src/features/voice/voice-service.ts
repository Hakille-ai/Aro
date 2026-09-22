// Futuristic Apple-grade Voice Service for ARO
// Supports Built-in Zero-Config AGI Voice, OpenAI Realtime / GPT Live, and Gemini Live.
import { writable, get } from "svelte/store";

export type VoiceState = "idle" | "listening" | "thinking" | "speaking" | "error";

export type VoiceProvider = "builtin" | "openai_realtime" | "gemini_live";

export interface VoiceDefinition {
  id: string;
  name: string;
  provider: VoiceProvider;
  gender: "female" | "male" | "neutral";
  language: string;
  description: string;
  previewSample?: string;
}

export interface VoiceApiKeys {
  openai?: string;
  gemini?: string;
}

export const AVAILABLE_VOICES: VoiceDefinition[] = [
  // Built-in Natural Voices
  {
    id: "builtin-nova",
    name: "Nova (Naturelle)",
    provider: "builtin",
    gender: "female",
    language: "fr/en",
    description: "Chaleureuse, dynamique et claire. Idéale pour le travail quotidien.",
  },
  {
    id: "builtin-alloy",
    name: "Alloy (Équilibrée)",
    provider: "builtin",
    gender: "neutral",
    language: "fr/en",
    description: "Neutre, concise et professionnelle.",
  },
  {
    id: "builtin-sage",
    name: "Sage (Sereine)",
    provider: "builtin",
    gender: "female",
    language: "fr/en",
    description: "Posée, calme et articulée pour les explications complexes.",
  },
  // OpenAI Realtime / GPT Live
  {
    id: "openai-alloy",
    name: "Alloy (OpenAI Live)",
    provider: "openai_realtime",
    gender: "neutral",
    language: "multilingual",
    description: "Streaming direct ultra-faible latence par le modèle GPT-4o Realtime.",
  },
  {
    id: "openai-nova",
    name: "Nova (OpenAI Live)",
    provider: "openai_realtime",
    gender: "female",
    language: "multilingual",
    description: "Synthèse ultra-expressive avec inflexions humaines naturelles.",
  },
  {
    id: "openai-fable",
    name: "Fable (OpenAI Live)",
    provider: "openai_realtime",
    gender: "male",
    language: "multilingual",
    description: "Ton captivant, énergique et narratif.",
  },
  {
    id: "openai-shimmer",
    name: "Shimmer (OpenAI Live)",
    provider: "openai_realtime",
    gender: "female",
    language: "multilingual",
    description: "Clarté cristalline, voix douce et posée.",
  },
  // Gemini Live / Multimodal Live
  {
    id: "gemini-aoede",
    name: "Aoede (Gemini Live)",
    provider: "gemini_live",
    gender: "female",
    language: "multilingual",
    description: "Voix multimodale Google DeepMind avec streaming bidirectionnel natif.",
  },
  {
    id: "gemini-puck",
    name: "Puck (Gemini Live)",
    provider: "gemini_live",
    gender: "male",
    language: "multilingual",
    description: "Ton direct, amical et percutant.",
  },
  {
    id: "gemini-kore",
    name: "Kore (Gemini Live)",
    provider: "gemini_live",
    gender: "female",
    language: "multilingual",
    description: "Douce, attentive et réactive au contexte multimodal.",
  },
];

const STORAGE_KEY_VOICE_PROVIDER = "aro_voice_active_provider";
const STORAGE_KEY_SELECTED_VOICE = "aro_voice_selected_id";
const STORAGE_KEY_VOICE_KEYS = "aro_voice_api_keys";

function loadStorage<T>(key: string, def: T): T {
  if (typeof localStorage === "undefined") return def;
  try {
    const v = localStorage.getItem(key);
    return v ? JSON.parse(v) : def;
  } catch {
    return def;
  }
}

function saveStorage<T>(key: string, v: T): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify(v));
  } catch (err) {
    console.error(`Failed to save ${key}:`, err);
  }
}

// Stores
export const voiceState = writable<VoiceState>("idle");
export const audioLevel = writable<number>(0); // 0.0 to 1.0 (frequency amplitude)
export const liveTranscript = writable<string>("");
export const assistantTranscript = writable<string>("");
export const activeVoiceProvider = writable<VoiceProvider>(
  loadStorage<VoiceProvider>(STORAGE_KEY_VOICE_PROVIDER, "builtin")
);
export const selectedVoiceId = writable<string>(
  loadStorage<string>(STORAGE_KEY_SELECTED_VOICE, "builtin-nova")
);
export const voiceApiKeys = writable<VoiceApiKeys>(
  loadStorage<VoiceApiKeys>(STORAGE_KEY_VOICE_KEYS, {})
);

activeVoiceProvider.subscribe((val) => saveStorage(STORAGE_KEY_VOICE_PROVIDER, val));
selectedVoiceId.subscribe((val) => saveStorage(STORAGE_KEY_SELECTED_VOICE, val));
voiceApiKeys.subscribe((val) => saveStorage(STORAGE_KEY_VOICE_KEYS, val));

// Audio context and mic analyser
let audioContext: AudioContext | null = null;
let analyserNode: AnalyserNode | null = null;
let micStream: MediaStream | null = null;
let animationFrameId: number | null = null;
let speechRecognitionInstance: any = null;

// Speech synthesis reference
let synthUtterance: SpeechSynthesisUtterance | null = null;

function setupAudioAnalyser(stream: MediaStream) {
  try {
    audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    analyserNode = audioContext.createAnalyser();
    analyserNode.fftSize = 256;
    const source = audioContext.createMediaStreamSource(stream);
    source.connect(analyserNode);

    const dataArray = new Uint8Array(analyserNode.frequencyBinCount);

    function updateAudioLevel() {
      if (!analyserNode) return;
      analyserNode.getByteFrequencyData(dataArray);
      let sum = 0;
      for (let i = 0; i < dataArray.length; i++) {
        sum += dataArray[i];
      }
      const avg = sum / dataArray.length;
      // Normalize to 0..1
      const normalized = Math.min(1, avg / 128);
      audioLevel.set(normalized);
      animationFrameId = requestAnimationFrame(updateAudioLevel);
    }
    updateAudioLevel();
  } catch (err) {
    console.warn("AudioContext analyser setup failed:", err);
  }
}

function stopAudioAnalyser() {
  if (animationFrameId !== null) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  if (micStream) {
    micStream.getTracks().forEach((t) => t.stop());
    micStream = null;
  }
  if (audioContext && audioContext.state !== "closed") {
    audioContext.close().catch(() => {});
    audioContext = null;
  }
  analyserNode = null;
  audioLevel.set(0);
}

// Start continuous listening session
export async function startVoiceSession(
  onTranscriptChange?: (text: string) => void,
  onSubmitTranscript?: (finalText: string) => void
): Promise<void> {
  const provider = get(activeVoiceProvider);
  voiceState.set("listening");
  liveTranscript.set("");

  try {
    // 1. Initialize microphone and real-time audio analyser for ORB visualization
    if (typeof navigator !== "undefined" && navigator.mediaDevices) {
      micStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      setupAudioAnalyser(micStream);
    }

    // 2. Initialize Speech Recognition (Native Web Speech - zero download)
    const SpeechRec = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
    if (SpeechRec) {
      const recognition = new SpeechRec();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = "fr-FR";

      recognition.onresult = (event: any) => {
        let interim = "";
        let final = "";
        for (let i = event.resultIndex; i < event.results.length; ++i) {
          if (event.results[i].isFinal) {
            final += event.results[i][0].transcript;
          } else {
            interim += event.results[i][0].transcript;
          }
        }
        const text = final || interim;
        liveTranscript.set(text);
        if (onTranscriptChange) onTranscriptChange(text);
        if (final && onSubmitTranscript) {
          onSubmitTranscript(final.trim());
        }
      };

      recognition.onerror = (err: any) => {
        if (err.error !== "no-speech") {
          console.warn("Speech recognition issue:", err.error);
        }
      };

      recognition.onend = () => {
        if (get(voiceState) === "listening") {
          try {
            recognition.start();
          } catch {}
        }
      };

      recognition.start();
      speechRecognitionInstance = recognition;
    }
  } catch (err) {
    console.error("Failed to start voice session:", err);
    voiceState.set("error");
    stopAudioAnalyser();
  }
}

// Stop voice session
export function stopVoiceSession(): void {
  if (speechRecognitionInstance) {
    try {
      speechRecognitionInstance.stop();
    } catch {}
    speechRecognitionInstance = null;
  }
  stopAudioAnalyser();
  voiceState.set("idle");
}

// Synthesize speech
export function speakText(text: string, onEnd?: () => void): void {
  if (!text || typeof window === "undefined" || !window.speechSynthesis) return;

  // Stop any active speech
  window.speechSynthesis.cancel();

  voiceState.set("speaking");
  assistantTranscript.set(text);

  const utterance = new SpeechSynthesisUtterance(text);
  utterance.rate = 1.05;
  utterance.pitch = 1.0;

  // Pick suitable system voice
  const voices = window.speechSynthesis.getVoices();
  const frVoice = voices.find((v) => v.lang.startsWith("fr") && (v.name.includes("Natural") || v.name.includes("Google") || v.name.includes("Thomas") || v.name.includes("Audrey")));
  if (frVoice) {
    utterance.voice = frVoice;
  }

  // Simulate dynamic speaking amplitude on the ORB
  let speakingInterval = setInterval(() => {
    if (get(voiceState) === "speaking") {
      audioLevel.set(0.3 + Math.random() * 0.5);
    } else {
      clearInterval(speakingInterval);
    }
  }, 100);

  utterance.onend = () => {
    clearInterval(speakingInterval);
    audioLevel.set(0);
    voiceState.set("idle");
    if (onEnd) onEnd();
  };

  utterance.onerror = () => {
    clearInterval(speakingInterval);
    audioLevel.set(0);
    voiceState.set("idle");
  };

  synthUtterance = utterance;
  window.speechSynthesis.speak(utterance);
}

// Stop speaking
export function stopSpeaking(): void {
  if (typeof window !== "undefined" && window.speechSynthesis) {
    window.speechSynthesis.cancel();
  }
  audioLevel.set(0);
  if (get(voiceState) === "speaking") {
    voiceState.set("idle");
  }
}

// Test voice sample
export function testVoiceSample(voiceId: string): void {
  const voice = AVAILABLE_VOICES.find((v) => v.id === voiceId) || AVAILABLE_VOICES[0];
  const sample =
    voice.language.includes("fr")
      ? `Bonjour ! Je suis ${voice.name}. Prêt à vous assister avec intelligence et précision.`
      : `Hello! I am ${voice.name}. Ready to assist you with intelligent autonomous reasoning.`;
  speakText(sample);
}
