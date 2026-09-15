import type {
  CloudSessionView,
  OrganizationInvitation,
  OrganizationMember,
} from "../../lib/types";

export interface UserProfile {
  name: string;
  email: string;
  roleTitle: string;
  avatarColor: string;
}

export interface OrganizationInfo {
  name: string;
  domain: string;
  description: string;
}

export interface OrgMember {
  id: string;
  userId?: string;
  cloudId?: string;
  invitationId?: string;
  invitationStatus?: OrganizationInvitation["status"];
  invitationExpiresAt?: string;
  invitationDeliveryStatus?: OrganizationInvitation["deliveryStatus"];
  name: string;
  email: string;
  role: "admin" | "manager" | "member" | "guest";
  status: "active" | "invited";
}

export interface OrgTeam {
  id: string;
  cloudId?: string;
  name: string;
  description: string;
  memberIds: string[];
}

export interface ApiKeyRecord {
  id: string;
  name: string;
  secret?: string;
  prefix?: string;
  lastUsedAt?: string | null;
  oneTimeSecret?: string;
  createdAt: string;
  visible: boolean;
}

export const DEFAULT_PROFILE_AVATAR_COLOR = "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)";

export function createInitialUserProfile(): UserProfile {
  return { name: "", email: "", roleTitle: "", avatarColor: DEFAULT_PROFILE_AVATAR_COLOR };
}

export function createInitialOrganizationInfo(): OrganizationInfo {
  return { name: "", domain: "", description: "" };
}

export function getInitials(name: string): string {
  if (!name) return "?";
  return name.split(/\s+/).map((part) => part[0]).slice(0, 2).join("").toUpperCase();
}

export function isCurrentOrgMember(member: OrgMember, currentUserId?: string | null): boolean {
  return member.id === "1" || (!!member.userId && member.userId === currentUserId);
}

export function filterOrganizationMembers(members: OrgMember[], query: string): OrgMember[] {
  const normalized = query.toLowerCase();
  return members.filter((member) =>
    member.name.toLowerCase().includes(normalized) || member.email.toLowerCase().includes(normalized),
  );
}

export function currentOrganizationRole(session: CloudSessionView | null): "admin" | "member" {
  if (!session) return "member";
  const membership = session.memberships.find((candidate) =>
    candidate.userId === session.user.id
    && candidate.organizationId === session.activeOrganization.id
    && candidate.status === "active",
  );
  return membership?.role === "owner" || membership?.role === "admin" ? "admin" : "member";
}

export function teamPayload(team: OrgTeam) {
  return { clientId: team.id, name: team.name, description: team.description, memberIds: team.memberIds };
}

export function mapOrganizationMember(member: OrganizationMember): OrgMember {
  return {
    id: member.id,
    cloudId: member.id,
    userId: member.userId,
    name: member.name,
    email: member.email,
    role: member.role === "owner" ? "admin" : member.role,
    status: member.status === "active" ? "active" : "invited",
  };
}

export function mergeOrganizationInvitations(
  members: OrganizationMember[],
  invitations: OrganizationInvitation[],
): OrgMember[] {
  const merged = members.map(mapOrganizationMember);
  for (const invitation of invitations) {
    const existingIndex = invitation.membershipId
      ? merged.findIndex((member) => member.cloudId === invitation.membershipId)
      : -1;
    const pendingMember: OrgMember = {
      id: invitation.membershipId ?? invitation.id,
      cloudId: invitation.membershipId ?? undefined,
      invitationId: invitation.id,
      invitationStatus: invitation.status,
      invitationExpiresAt: invitation.expiresAt,
      invitationDeliveryStatus: invitation.deliveryStatus,
      name: invitation.name,
      email: invitation.email,
      role: invitation.role,
      status: "invited",
    };
    if (existingIndex >= 0) {
      merged[existingIndex] = { ...merged[existingIndex], ...pendingMember, userId: merged[existingIndex].userId };
    } else {
      merged.push(pendingMember);
    }
  }
  return merged;
}

export function invitationStatusLabel(member: OrgMember, language: "fr" | "en"): string {
  if (member.invitationStatus === "expired") return language === "fr" ? "Expirée" : "Expired";
  if (member.invitationStatus === "failed") return language === "fr" ? "Échec d’envoi" : "Delivery failed";
  if (member.invitationStatus === "delivered") return language === "fr" ? "Invitation envoyée" : "Invitation sent";
  return language === "fr" ? "Invitation en attente" : "Invitation pending";
}

export function mapApiKeyRecord(key: {
  id: string;
  name: string;
  prefix: string;
  createdAt: string;
  lastUsedAt?: string | null;
}): ApiKeyRecord {
  return {
    id: key.id,
    name: key.name,
    prefix: key.prefix,
    createdAt: key.createdAt.split("T")[0],
    lastUsedAt: key.lastUsedAt ?? null,
    visible: false,
  };
}
