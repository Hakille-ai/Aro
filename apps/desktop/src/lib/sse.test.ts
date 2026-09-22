import { describe, it, expect } from "vitest";
import { createSseParser } from "./sse";

describe("createSseParser", () => {
  it("keeps the event type when event: and data: arrive in separate reads", () => {
    const parser = createSseParser();
    expect(parser.feed('event: chunk\n')).toEqual([]);
    const events = parser.feed('data: {"content":"hello"}\n\n');
    expect(events).toHaveLength(1);
    expect(events[0].kind).toBe("chunk");
    expect(events[0].data).toEqual({ content: "hello" });
  });

  it("handles a split inside the data line itself", () => {
    const parser = createSseParser();
    expect(parser.feed('event: step\ndata: {"ste')).toEqual([]);
    const events = parser.feed('p": 1}\n\n');
    expect(events).toHaveLength(1);
    expect(events[0].kind).toBe("step");
  });

  it("dispatches multiple events from a single read", () => {
    const parser = createSseParser();
    const events = parser.feed(
      'event: chunk\ndata: {"content":"a"}\n\nevent: done\ndata: {"ok":true}\n\n'
    );
    expect(events.map((e) => e.kind)).toEqual(["chunk", "done"]);
  });

  it("ignores SSE comments and blank noise", () => {
    const parser = createSseParser();
    const events = parser.feed(': heartbeat\n\nevent: chunk\ndata: {"content":"x"}\n\n');
    expect(events).toHaveLength(1);
    expect(events[0].kind).toBe("chunk");
  });

  it("drops data lines without an event type instead of misrouting them", () => {
    const parser = createSseParser();
    const events = parser.feed('data: {"content":"orphan"}\n\n');
    expect(events).toHaveLength(1);
    expect(events[0].kind).toBe("");
  });

  it("skips malformed JSON and keeps parsing afterwards", () => {
    const errors: unknown[] = [];
    const parser = createSseParser((e) => errors.push(e));
    const events = parser.feed(
      'event: chunk\ndata: not-json\n\nevent: chunk\ndata: {"content":"ok"}\n\n'
    );
    expect(errors).toHaveLength(1);
    expect(events).toHaveLength(1);
    expect(events[0].data).toEqual({ content: "ok" });
  });

  it("flushes a trailing unterminated event at stream end", () => {
    const parser = createSseParser();
    expect(parser.feed('event: done\ndata: {"ok":true}')).toEqual([]);
    const events = parser.flush();
    expect(events).toHaveLength(1);
    expect(events[0].kind).toBe("done");
  });
});
