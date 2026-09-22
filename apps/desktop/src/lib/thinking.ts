export interface ParsedThinking {
  reasoning: string;
  isReasoningComplete: boolean;
  actualContent: string;
  hasReasoning: boolean;
}

/**
 * Parses content containing one or multiple <think>...</think> tags.
 * Works seamlessly with full content, multi-turn reasoning loops, and real-time stream chunks.
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

  const thinkPairs = [
    { start: "<think>", end: "</think>" },
    { start: "<thought>", end: "</thought>" },
  ];

  const thoughts: string[] = [];
  let cleanContent = "";
  let isCurrentlyThinking = false;
  let cursor = 0;

  while (cursor < content.length) {
    // Find the earliest starting tag among supported pairs
    let earliestStart = -1;
    let activePair = thinkPairs[0];

    for (const pair of thinkPairs) {
      const idx = content.indexOf(pair.start, cursor);
      if (idx !== -1 && (earliestStart === -1 || idx < earliestStart)) {
        earliestStart = idx;
        activePair = pair;
      }
    }

    if (earliestStart === -1) {
      // No more think tags in remaining content
      cleanContent += content.slice(cursor);
      break;
    }

    // Append any text before this thinking block to cleanContent
    cleanContent += content.slice(cursor, earliestStart);

    // Look for matching closing tag
    const endIdx = content.indexOf(activePair.end, earliestStart + activePair.start.length);
    if (endIdx === -1) {
      // Unclosed thinking tag: model is still actively streaming reasoning
      const unclosedThought = content.slice(earliestStart + activePair.start.length).trim();
      if (unclosedThought) {
        thoughts.push(unclosedThought);
      }
      isCurrentlyThinking = true;
      cursor = content.length;
      break;
    }

    // Completed thinking block
    const thought = content.slice(earliestStart + activePair.start.length, endIdx).trim();
    if (thought) {
      thoughts.push(thought);
    }
    cursor = endIdx + activePair.end.length;
  }

  // Sanitize actualContent: remove any stray/unmatched closing or opening tags
  let actualContent = cleanContent
    .replace(/<\/think>/gi, "")
    .replace(/<think>/gi, "")
    .replace(/<\/thought>/gi, "")
    .replace(/<thought>/gi, "")
    .trim();

  const reasoning = thoughts.join("\n\n").trim();

  // Guard against models that accidentally output their thought as content
  if (actualContent && reasoning && (actualContent === reasoning || thoughts.includes(actualContent))) {
    actualContent = "";
  }

  const hasReasoning = reasoning.length > 0;
  return {
    reasoning,
    isReasoningComplete: hasReasoning ? !isCurrentlyThinking : false,
    actualContent,
    hasReasoning,
  };
}
