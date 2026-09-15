use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MembershipRole {
    Owner,
    Admin,
    Manager,
    Member,
    Guest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MembershipStatus {
    Active,
    Invited,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role_title: Option<String>,
    pub avatar_color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub role: MembershipRole,
    pub status: MembershipStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationMember {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub email: String,
    pub role: MembershipRole,
    pub status: MembershipStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InvitationDeliveryStatus {
    Pending,
    Processing,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OrganizationInvitationStatus {
    Pending,
    Delivered,
    Failed,
    Expired,
    Accepted,
    Revoked,
}

/// Safe invitation metadata visible to tenant administrators. Bearer credentials, encrypted
/// delivery material, provider errors, and password/account internals are deliberately absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationInvitation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub membership_id: Option<Uuid>,
    pub email: String,
    pub name: String,
    pub role: MembershipRole,
    pub status: OrganizationInvitationStatus,
    pub delivery_status: InvitationDeliveryStatus,
    pub delivery_attempts: i32,
    pub expires_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub platform: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub user: User,
    pub active_organization: Organization,
    pub memberships: Vec<Membership>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}
