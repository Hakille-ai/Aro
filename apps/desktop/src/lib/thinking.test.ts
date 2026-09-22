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

  it("suppresses actualContent if it erroneously duplicates reasoning", () => {
    const content = "<think>\nOkay, the user said salut\n</think>\nOkay, the user said salut";
    expect(parseMessageThinking(content)).toEqual({
      reasoning: "Okay, the user said salut",
      isReasoningComplete: true,
      actualContent: "",
      hasReasoning: true,
    });
  });

  it("handles multiple thinking blocks from multi-step reasoning models", () => {
    const content =
      "<think>Initial plan: search news.</think>" +
      "<think>The search tool was blocked due to network access being disabled. I will explain to user.</think>" +
      "\nDésolé Diallo, je ne peux pas accéder à Internet pour le moment.";

    const result = parseMessageThinking(content);
    expect(result.hasReasoning).toBe(true);
    expect(result.isReasoningComplete).toBe(true);
    expect(result.reasoning).toBe(
      "Initial plan: search news.\n\nThe search tool was blocked due to network access being disabled. I will explain to user."
    );
    expect(result.actualContent).toBe("Désolé Diallo, je ne peux pas accéder à Internet pour le moment.");
    expect(result.actualContent).not.toContain("<think>");
    expect(result.actualContent).not.toContain("</think>");
  });

  it("handles multiple thinking blocks when the last one is still streaming", () => {
    const content =
      "<think>First thought complete</think>" +
      "Some intermediate status" +
      "<think>Second thought still streaming...";

    const result = parseMessageThinking(content);
    expect(result.hasReasoning).toBe(true);
    expect(result.isReasoningComplete).toBe(false);
    expect(result.reasoning).toBe("First thought complete\n\nSecond thought still streaming...");
    expect(result.actualContent).toBe("Some intermediate status");
  });
});
