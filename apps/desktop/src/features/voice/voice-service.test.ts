import { describe, expect, it, beforeEach } from "vitest";
import { get } from "svelte/store";
import {
  voiceState,
  audioLevel,
  activeVoiceProvider,
  selectedVoiceId,
  voiceApiKeys,
  AVAILABLE_VOICES,
  liveTranscript,
  assistantTranscript,
  stopVoiceSession,
  stopSpeaking,
} from "./voice-service";

describe("voice-service", () => {
  beforeEach(() => {
    voiceState.set("idle");
    audioLevel.set(0);
    liveTranscript.set("");
    assistantTranscript.set("");
    activeVoiceProvider.set("builtin");
    selectedVoiceId.set("builtin-nova");
    voiceApiKeys.set({});
  });

  it("contains curated multi-provider voices including OpenAI Realtime and Gemini Live", () => {
    expect(AVAILABLE_VOICES.length).toBeGreaterThan(5);

    const builtinVoices = AVAILABLE_VOICES.filter((v) => v.provider === "builtin");
    const openaiVoices = AVAILABLE_VOICES.filter((v) => v.provider === "openai_realtime");
    const geminiVoices = AVAILABLE_VOICES.filter((v) => v.provider === "gemini_live");

    expect(builtinVoices.length).toBeGreaterThanOrEqual(2);
    expect(openaiVoices.length).toBeGreaterThanOrEqual(2);
    expect(geminiVoices.length).toBeGreaterThanOrEqual(2);

    expect(openaiVoices.some((v) => v.id.includes("alloy"))).toBe(true);
    expect(geminiVoices.some((v) => v.id.includes("puck") || v.id.includes("aoede"))).toBe(true);
  });

  it("switches providers and updates selected voice ID", () => {
    activeVoiceProvider.set("openai_realtime");
    expect(get(activeVoiceProvider)).toBe("openai_realtime");

    selectedVoiceId.set("openai-nova");
    expect(get(selectedVoiceId)).toBe("openai-nova");

    activeVoiceProvider.set("gemini_live");
    expect(get(activeVoiceProvider)).toBe("gemini_live");
  });

  it("stores and retrieves external provider API keys", () => {
    voiceApiKeys.set({
      openai: "sk-test-key-12345",
      gemini: "AIzaSyTestKey67890",
    });

    const keys = get(voiceApiKeys);
    expect(keys.openai).toBe("sk-test-key-12345");
    expect(keys.gemini).toBe("AIzaSyTestKey67890");
  });

  it("manages voice states correctly", () => {
    expect(get(voiceState)).toBe("idle");

    voiceState.set("listening");
    expect(get(voiceState)).toBe("listening");

    voiceState.set("thinking");
    expect(get(voiceState)).toBe("thinking");

    voiceState.set("speaking");
    expect(get(voiceState)).toBe("speaking");

    stopSpeaking();
    expect(get(voiceState)).toBe("idle");
  });
});
