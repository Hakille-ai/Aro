import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function cloudSession(organizationId: string, accessToken: string, refreshToken: string) {
  return {
    accessToken,
    refreshToken,
    expiresAt: "2026-07-12T12:00:00Z",
    user: {
      id: "user-id",
      email: "user@example.com",
      name: "ARO User",
      roleTitle: null,
      avatarColor: null,
      createdAt: "2026-07-12T10:00:00Z",
      updatedAt: "2026-07-12T10:00:00Z",
    },
    activeOrganization: {
      id: organizationId,
      name: `Organization ${organizationId}`,
      slug: organizationId,
      domain: null,
      description: null,
      createdAt: "2026-07-12T10:00:00Z",
      updatedAt: "2026-07-12T10:00:00Z",
    },
    memberships: [],
  };
}

describe("web cloud authentication", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", {});
    vi.stubGlobal("localStorage", { removeItem: vi.fn() });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("rotates the complete session when switching organization", async () => {
    const fetchMock = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-1", "refresh-1")))
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-2", "access-2", "refresh-2")))
      .mockResolvedValueOnce(jsonResponse({ ok: true }));
    vi.stubGlobal("fetch", fetchMock);

    const { loginCloud, logoutCloud, switchCloudOrganization } = await import("./api");
    await loginCloud({ email: "user@example.com", password: "password" });

    const switched = await switchCloudOrganization("org-2");
    expect(switched.activeOrganization.id).toBe("org-2");
    expect(JSON.parse(String(fetchMock.mock.calls[1][1]?.body))).toEqual({
      organizationId: "org-2",
      refreshToken: "refresh-1",
    });

    await logoutCloud();
    expect(JSON.parse(String(fetchMock.mock.calls[2][1]?.body))).toEqual({
      refreshToken: "refresh-2",
    });
  });

  it("refreshes first and rebuilds a switch request after an expired access token", async () => {
    const fetchMock = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-1", "refresh-1")))
      .mockResolvedValueOnce(jsonResponse({ error: "invalid or expired token" }, 401))
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-2", "refresh-2")))
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-2", "access-3", "refresh-3")));
    vi.stubGlobal("fetch", fetchMock);

    const { loginCloud, switchCloudOrganization } = await import("./api");
    await loginCloud({ email: "user@example.com", password: "password" });
    const switched = await switchCloudOrganization("org-2");

    expect(switched.activeOrganization.id).toBe("org-2");
    expect(JSON.parse(String(fetchMock.mock.calls[1][1]?.body))).toEqual({
      organizationId: "org-2",
      refreshToken: "refresh-1",
    });
    expect(JSON.parse(String(fetchMock.mock.calls[2][1]?.body))).toEqual({
      refreshToken: "refresh-1",
    });
    expect(JSON.parse(String(fetchMock.mock.calls[3][1]?.body))).toEqual({
      organizationId: "org-2",
      refreshToken: "refresh-2",
    });
  });

  it("propagates a logout server failure after clearing the local session", async () => {
    const fetchMock = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-1", "refresh-1")))
      .mockResolvedValueOnce(jsonResponse({ error: "logout unavailable" }, 503));
    vi.stubGlobal("fetch", fetchMock);

    const { getCloudSession, loginCloud, logoutCloud } = await import("./api");
    await loginCloud({ email: "user@example.com", password: "password" });

    await expect(logoutCloud()).rejects.toThrow("logout unavailable");
    expect(await getCloudSession()).toBeNull();
    expect(JSON.parse(String(fetchMock.mock.calls[1][1]?.body))).toEqual({
      refreshToken: "refresh-1",
    });
  });

  it("loads every invitation page and forwards the opaque cursor", async () => {
    const fetchMock = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-1", "refresh-1")))
      .mockResolvedValueOnce(jsonResponse({ items: [{ id: "invite-1" }], nextCursor: "cursor-1" }))
      .mockResolvedValueOnce(jsonResponse({ items: [{ id: "invite-2" }], nextCursor: null }));
    vi.stubGlobal("fetch", fetchMock);

    const { listAllCloudInvitations, loginCloud } = await import("./api");
    await loginCloud({ email: "user@example.com", password: "password" });
    const invitations = await listAllCloudInvitations(false);

    expect(invitations.map((invitation) => invitation.id)).toEqual(["invite-1", "invite-2"]);
    expect(String(fetchMock.mock.calls[1][0])).not.toContain("cursor=");
    expect(String(fetchMock.mock.calls[2][0])).toContain("cursor=cursor-1");
  });

  it("stops invitation pagination when the server repeats a cursor", async () => {
    const fetchMock = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse(cloudSession("org-1", "access-1", "refresh-1")))
      .mockResolvedValueOnce(jsonResponse({ items: [], nextCursor: "cursor-1" }))
      .mockResolvedValueOnce(jsonResponse({ items: [], nextCursor: "cursor-1" }));
    vi.stubGlobal("fetch", fetchMock);

    const { listAllCloudInvitations, loginCloud } = await import("./api");
    await loginCloud({ email: "user@example.com", password: "password" });

    await expect(listAllCloudInvitations(false)).rejects.toThrow("repeated cursor");
  });
});
