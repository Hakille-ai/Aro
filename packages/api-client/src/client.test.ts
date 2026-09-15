import { describe, it, expect, vi, beforeEach } from "vitest";
import { AroApiClient } from "./client";
import { MemoryTokenStorage } from "./storage";
import { ApiError } from "./errors";

describe("AroApiClient", () => {
  let storage: MemoryTokenStorage;
  let client: AroApiClient;

  beforeEach(() => {
    storage = new MemoryTokenStorage();
    client = new AroApiClient({
      baseUrl: "http://127.0.0.1:59999",
      tokenStorage: storage,
      enableMockFallback: true,
    });
  });

  it("normalizes base URL properly", () => {
    const c = new AroApiClient({ baseUrl: "http://localhost:8080///" });
    expect(c.getBaseUrl()).toBe("http://localhost:8080");
  });

  it("stores and clears authentication tokens", async () => {
    await storage.setAccessToken("test-access-token");
    await storage.setRefreshToken("test-refresh-token");

    expect(await storage.getAccessToken()).toBe("test-access-token");
    expect(await storage.getRefreshToken()).toBe("test-refresh-token");

    await storage.clear();
    expect(await storage.getAccessToken()).toBeNull();
    expect(await storage.getRefreshToken()).toBeNull();
  });

  it("formats ApiError accurately from status and json error response", () => {
    const err = ApiError.fromResponse(404, { error: "Conversation not found", code: "NOT_FOUND" });
    expect(err.status).toBe(404);
    expect(err.message).toBe("Conversation not found");
    expect(err.code).toBe("NOT_FOUND");
  });

  it("provides reliable mock fallback when server is offline and enableMockFallback is true", async () => {
    const conversations = await client.listConversations();
    expect(Array.isArray(conversations)).toBe(true);
    expect(conversations.length).toBeGreaterThan(0);
    expect(conversations[0].title).toBeDefined();

    const plans = await client.listPlans();
    expect(Array.isArray(plans)).toBe(true);
    expect(plans[0].tasks.length).toBeGreaterThan(0);

    const models = await client.listModels();
    expect(Array.isArray(models)).toBe(true);
    expect(models.some((m) => m.provider === "anthropic")).toBe(true);
  });

  it("formats createConversation request with mode=chat for Axum backend compatibility", async () => {
    let capturedBody: any = null;
    vi.spyOn(globalThis, "fetch").mockImplementationOnce(async (_url, init) => {
      capturedBody = JSON.parse(init?.body as string);
      return new Response(
        JSON.stringify({
          id: "conv-100",
          title: "New Chat",
          mode: "chat",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    });

    const created = await client.createConversation({ title: "New Chat" });
    expect(capturedBody).toBeDefined();
    expect(capturedBody.mode).toBe("chat");
    expect(capturedBody.title).toBe("New Chat");
    expect(created.id).toBe("conv-100");
  });

  it("parses Axum SSE chunk and done events properly in streamAssistant", async () => {
    const ssePayload = [
      "event: chunk\n",
      'data: {"content":"Hello "}\n\n',
      "event: chunk\n",
      'data: {"content":"world!"}\n\n',
      "event: done\n",
      'data: {"conversation":{"id":"conv-1"},"assistantMessage":{"content":"Hello world!"}}\n\n',
    ].join("");

    vi.spyOn(globalThis, "fetch").mockImplementationOnce(async () => {
      const stream = new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(ssePayload));
          controller.close();
        },
      });
      return new Response(stream, {
        status: 200,
        headers: { "Content-Type": "text/event-stream" },
      });
    });

    const receivedChunks: string[] = [];
    let doneCalled = false;

    await new Promise<void>((resolve) => {
      client.streamAssistant(
        { conversationId: "conv-1", content: "Hi", modelId: "claude-3-7-sonnet" },
        {
          onChunk: (chunk) => receivedChunks.push(chunk),
          onDone: () => {
            doneCalled = true;
            resolve();
          },
        }
      );
    });

    expect(receivedChunks).toEqual(["Hello ", "world!"]);
    expect(doneCalled).toBe(true);
  });
});
