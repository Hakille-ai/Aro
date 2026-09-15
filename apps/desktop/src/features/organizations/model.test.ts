import { describe, expect, it } from "vitest";
import type { OrganizationInvitation, OrganizationMember } from "../../lib/types";
import {
  DEFAULT_PROFILE_AVATAR_COLOR,
  createInitialOrganizationInfo,
  createInitialUserProfile,
  filterOrganizationMembers,
  getInitials,
  invitationStatusLabel,
  isCurrentOrgMember,
  mapApiKeyRecord,
  mapOrganizationMember,
  mergeOrganizationInvitations,
  teamPayload,
  type OrgMember,
} from "./model";

const member = (overrides: Partial<OrganizationMember> = {}): OrganizationMember => ({
  id: "membership-1", organizationId: "org-1", userId: "user-1", name: "Alice Martin",
  email: "alice@example.com", role: "member", status: "active", createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  ...overrides,
});

const invitation = (overrides: Partial<OrganizationInvitation> = {}): OrganizationInvitation => ({
  id: "invite-1", organizationId: "org-1", membershipId: null, email: "bob@example.com", name: "Bob",
  role: "guest", status: "pending", deliveryStatus: "pending", expiresAt: "2026-08-01T00:00:00Z",
  deliveryAttempts: 0, deliveredAt: null, createdAt: "2026-07-01T00:00:00Z",
  updatedAt: "2026-07-01T00:00:00Z", ...overrides,
});

describe("organization model", () => {
  it("preserves initial profile and organization values", () => {
    expect(createInitialUserProfile()).toEqual({ name: "", email: "", roleTitle: "", avatarColor: DEFAULT_PROFILE_AVATAR_COLOR });
    expect(createInitialOrganizationInfo()).toEqual({ name: "", domain: "", description: "" });
    expect(createInitialUserProfile()).not.toBe(createInitialUserProfile());
  });

  it.each([["", "?"], ["Ada", "A"], ["Ada Lovelace", "AL"], ["jean luc picard", "JL"]])(
    "formats initials for %j", (name, expected) => expect(getInitials(name)).toBe(expected),
  );

  it("filters members case-insensitively by name or email without reordering", () => {
    const members = [mapOrganizationMember(member()), mapOrganizationMember(member({ id: "2", name: "Bob", email: "team@EXAMPLE.org" }))];
    expect(filterOrganizationMembers(members, "example").map((item) => item.name)).toEqual(["Alice Martin", "Bob"]);
    expect(filterOrganizationMembers(members, "BOB").map((item) => item.id)).toEqual(["2"]);
  });

  it("preserves the legacy current-member id and user-id rules", () => {
    const base = mapOrganizationMember(member());
    expect(isCurrentOrgMember({ ...base, id: "1" }, null)).toBe(true);
    expect(isCurrentOrgMember(base, "user-1")).toBe(true);
    expect(isCurrentOrgMember(base, "other")).toBe(false);
  });

  it("maps owner memberships to admin", () => {
    expect(mapOrganizationMember(member({ role: "owner" }))).toMatchObject({ role: "admin", status: "active", cloudId: "membership-1" });
    expect(mapOrganizationMember(member({ status: "invited" }))).toMatchObject({ status: "invited" });
  });

  it("merges invitations in order and retains an existing user id", () => {
    const merged = mergeOrganizationInvitations(
      [member(), member({ id: "membership-2", userId: "user-2", name: "Claire", email: "claire@example.com" })],
      [invitation({ membershipId: "membership-1", name: "Alice Pending" }), invitation({ id: "invite-2", email: "new@example.com" })],
    );
    expect(merged.map((item) => item.id)).toEqual(["membership-1", "membership-2", "invite-2"]);
    expect(merged[0]).toMatchObject({ name: "Alice Pending", userId: "user-1", status: "invited", invitationId: "invite-1" });
  });

  it("keeps invitation status labels exactly localized", () => {
    const base = mapOrganizationMember(member()) as OrgMember;
    expect(invitationStatusLabel({ ...base, invitationStatus: "expired" }, "fr")).toBe("Expirée");
    expect(invitationStatusLabel({ ...base, invitationStatus: "failed" }, "en")).toBe("Delivery failed");
    expect(invitationStatusLabel({ ...base, invitationStatus: "delivered" }, "fr")).toBe("Invitation envoyée");
    expect(invitationStatusLabel(base, "en")).toBe("Invitation pending");
  });

  it("builds team and API-key payloads without reordering members", () => {
    expect(teamPayload({ id: "team-1", name: "Core", description: "Main", memberIds: ["b", "a"] })).toEqual({
      clientId: "team-1", name: "Core", description: "Main", memberIds: ["b", "a"],
    });
    expect(mapApiKeyRecord({ id: "key-1", name: "CI", prefix: "aro_", createdAt: "2026-07-15T12:00:00Z" })).toEqual({
      id: "key-1", name: "CI", prefix: "aro_", createdAt: "2026-07-15", lastUsedAt: null, visible: false,
    });
  });
});
