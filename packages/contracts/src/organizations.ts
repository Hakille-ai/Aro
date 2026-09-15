export type MembershipRole = "owner" | "admin" | "member" | "viewer";

export interface Organization {
  id: string;
  name: string;
  slug?: string;
  createdAt: string;
  updatedAt?: string;
  role?: MembershipRole;
  membersCount?: number;
}

export interface OrganizationMember {
  id: string;
  organizationId: string;
  userId: string;
  email: string;
  fullName?: string | null;
  role: MembershipRole;
  joinedAt: string;
}

export interface OrganizationInvitation {
  id: string;
  organizationId: string;
  organizationName: string;
  email: string;
  role: MembershipRole;
  invitedByEmail?: string;
  createdAt: string;
  expiresAt: string;
}

export interface OrganizationCreateRequest {
  name: string;
}

export interface OrganizationPatch {
  name?: string;
}

export interface MembershipCreateRequest {
  email: string;
  role: MembershipRole;
}
