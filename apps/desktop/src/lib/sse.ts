/**
 * Minimal spec-shaped SSE parser for ARO web streaming.
 *
 * The previous inline parser reset `currentEvent` on every network read, so
 * an `event:` line and its `data:` line landing in two different TCP chunks
 * lost the event type (chunks silently dropped). This parser keeps
 * buffer + current event type across `feed()` calls and dispatches one
 * event per blank line, per the SSE framing rules.
 */

export type SseEventKind = "chunk" | "step" | "done" | string;

export interface SseParsedEvent {
  kind: SseEventKind;
  data: unknown;
  raw: string;
}

export interface SseParser {
  /** Feed a decoded text fragment; returns fully-framed events. */
  feed(text: string): SseParsedEvent[];
  /** Drain a trailing unterminated line (stream end). */
  flush(): SseParsedEvent[];
}

export function createSseParser(onError?: (error: unknown, raw: string) => void): SseParser {
  let buffer = "";
  let currentEvent = "";

  function dispatch(dataLines: string[]): SseParsedEvent | null {
    const raw = dataLines.join("\n");
    if (!raw) {
      currentEvent = "";
      return null;
    }
    let data: unknown = null;
    try {
      data = JSON.parse(raw);
    } catch (error) {
      onError?.(error, raw);
      currentEvent = "";
      return null;
    }
    const event: SseParsedEvent = { kind: currentEvent, data, raw };
    currentEvent = "";
    return event;
  }

  function consume(final = false): SseParsedEvent[] {
    const out: SseParsedEvent[] = [];
    // A trailing partial line is kept for the next feed, unless flushing.
    const lines = buffer.split("\n");
    buffer = final ? "" : lines.pop() ?? "";
    let pending: string[] = [];
    const push = (line: string) => {
      const trimmed = line.trim();
      if (!trimmed) {
        const event = dispatch(pending);
        pending = [];
        if (event) out.push(event);
        return;
      }
      if (trimmed.startsWith(":")) return; // SSE comment / heartbeat
      if (trimmed.startsWith("event:")) {
        // A new event line implicitly closes an open one without blank line.
        if (pending.length > 0) {
          const event = dispatch(pending);
          pending = [];
          if (event) out.push(event);
        }
        currentEvent = trimmed.slice(6).trim();
      } else if (trimmed.startsWith("data:")) {
        pending.push(trimmed.slice(5).trim());
      }
    };
    for (const line of lines) push(line);
    if (final && pending.length > 0) {
      const event = dispatch(pending);
      if (event) out.push(event);
    }
    return out;
  }

  return {
    feed(text: string): SseParsedEvent[] {
      buffer += text;
      return consume(false);
    },
    flush(): SseParsedEvent[] {
      return consume(true);
    },
  };
}
