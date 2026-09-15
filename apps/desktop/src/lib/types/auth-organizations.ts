export type MembershipRole = "owner" | "admin" | "manager" | "member" | "guest";

export type MembershipStatus = "active" | "invited" | "suspended";

export type InferenceMode = "local" | "cloud";

export type SyncHealth = "online" | "offline-read-only" | "sync-pending" | "session-expired";

export interface User {
  id: string;
  email: string;
  name: string;
  roleTitle?: string | null;
  avatarColor?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Organization {
  id: string;
  name: string;
  domain?: string | null;
  description?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface UserProfilePatch {
  name?: string | null;
  roleTitle?: string | null;
  avatarColor?: string | null;
}

export interface OrganizationPatch {
  name?: string | null;
  domain?: string | null;
  description?: string | null;
}

export interface OrganizationCreateRequest {
  name: string;
  domain?: string | null;
  description?: string | null;
}

export interface PublicApiKey {
  id: string;
  organizationId: string;
  name: string;
  prefix: string;
  createdAt: string;
  lastUsedAt?: string | null;
}

export interface ApiKeyCreateResponse {
  key: PublicApiKey;
  secret: string;
}

export interface Membership {
  id: string;
  userId: string;
  organizationId: string;
  role: MembershipRole;
  status: MembershipStatus;
  createdAt: string;
  updatedAt: string;
}

export interface OrganizationMember {
  id: string;
  userId: string;
  organizationId: string;
  name: string;
  email: string;
  role: MembershipRole;
  status: MembershipStatus;
  createdAt: string;
  updatedAt: string;
}

export interface MembershipCreateRequest {
  name: string;
  email: string;
  role: Exclude<MembershipRole, "owner">;
}

export interface OrganizationInvitationReceipt {
  invitationId: string;
  organizationId: string;
  email: string;
  name: string;
  role: Exclude<MembershipRole, "owner">;
  status: "pending";
  expiresAt: string;
  member?: OrganizationMember | null;
}

export type InvitationDeliveryStatus = "pending" | "processing" | "delivered" | "failed";

export type OrganizationInvitationStatus =
  | "pending"
  | "delivered"
  | "failed"
  | "expired"
  | "accepted"
  | "revoked";

export interface OrganizationInvitation {
  id: string;
  organizationId: string;
  membershipId: string | null;
  email: string;
  name: string;
  role: Exclude<MembershipRole, "owner">;
  status: OrganizationInvitationStatus;
  deliveryStatus: InvitationDeliveryStatus;
  deliveryAttempts: number;
  expiresAt: string;
  deliveredAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface OrganizationInvitationPage {
  items: OrganizationInvitation[];
  nextCursor: string | null;
}

export interface UserPreferences {
  userId: string;
  theme: string;
  language: string;
  wakeWordEnabled: boolean;
  inferenceMode: InferenceMode;
  updatedAt: string;
}

export interface UserPreferencesPatch {
  theme?: string | null;
  language?: string | null;
  wakeWordEnabled?: boolean | null;
  inferenceMode?: InferenceMode | null;
}

export interface SyncStatus {
  health: SyncHealth;
  pendingEvents: number;
  lastSyncedAt?: string | null;
}

export interface CloudSessionView {
  user: User;
  activeOrganization: Organization;
  memberships: Membership[];
  expiresAt: string;
}

export interface CloudRegisterRequest {
  email: string;
  password: string;
  name: string;
  organizationName: string;
  organizationDomain?: string | null;
  organizationDescription?: string | null;
}

export interface CloudLoginRequest {
  email: string;
  password: string;
}
