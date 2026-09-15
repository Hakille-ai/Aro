export interface ParsedThinking {
  reasoning: string;
  isReasoningComplete: boolean;
  actualContent: string;
  hasReasoning: boolean;
}

/**
 * Parses content containing <think>...</think> tags.
 * Works with full content or real-time stream chunks.
 */
export function parseMessageThinking(content: string | undefined | null): ParsedThinking {
  if (!content) {
    return {
      reasoning: "",
      isReasoningComplete: false,
      actualContent: "",
      hasReasoning: false,
    };
  }

  const thinkStartTag = "<think>";
  const thinkEndTag = "</think>";

  const startIdx = content.indexOf(thinkStartTag);
  if (startIdx !== -1) {
    const endIdx = content.indexOf(thinkEndTag);
    if (endIdx !== -1) {
      const reasoning = content.slice(startIdx + thinkStartTag.length, endIdx).trim();
      const actualContent = content.slice(endIdx + thinkEndTag.length).trim();
      return {
        reasoning,
        isReasoningComplete: true,
        actualContent,
        hasReasoning: reasoning.length > 0,
      };
    } else {
      const reasoning = content.slice(startIdx + thinkStartTag.length).trim();
      return {
        reasoning,
        isReasoningComplete: false,
        actualContent: "",
        hasReasoning: reasoning.length > 0,
      };
    }
  }

  // Fallback: If no explicit tags but the message starts with a reasoning block
  // (e.g. if the model started thinking without emitting the tag or it was stripped)
  return {
    reasoning: "",
    isReasoningComplete: false,
    actualContent: content,
    hasReasoning: false,
  };
}
