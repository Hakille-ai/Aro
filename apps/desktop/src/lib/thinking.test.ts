import { describe, expect, it } from "vitest";
import { parseMessageThinking } from "./thinking";

describe("parseMessageThinking", () => {
  it("returns defaults for empty content", () => {
    expect(parseMessageThinking("")).toEqual({
      reasoning: "",
      isReasoningComplete: false,
      actualContent: "",
      hasReasoning: false,
    });
    expect(parseMessageThinking(null)).toEqual({
      reasoning: "",
      isReasoningComplete: false,
      actualContent: "",
      hasReasoning: false,
    });
  });

  it("parses fully complete thinking blocks", () => {
    const content = "<think>\nThis is a thought.\n</think>\nThis is the actual response.";
    expect(parseMessageThinking(content)).toEqual({
      reasoning: "This is a thought.",
      isReasoningComplete: true,
      actualContent: "This is the actual response.",
      hasReasoning: true,
    });
  });

  it("parses incomplete/streaming thinking blocks", () => {
    const content = "<think>\nThinking in progress...";
    expect(parseMessageThinking(content)).toEqual({
      reasoning: "Thinking in progress...",
      isReasoningComplete: false,
      actualContent: "",
      hasReasoning: true,
    });
  });

  it("returns content unchanged if no thinking block is present", () => {
    const content = "Hello world! This is a normal message.";
    expect(parseMessageThinking(content)).toEqual({
      reasoning: "",
      isReasoningComplete: false,
      actualContent: content,
      hasReasoning: false,
    });
  });
});
