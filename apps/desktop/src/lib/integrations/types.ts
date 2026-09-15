export type IntegrationKind =
  | "app"
  | "model-provider"
  | "mcp-server"
  | "webhook"
  | "scheduler"
  | "custom";

export type IntegrationAuthMode =
  | "none"
  | "oauth2"
  | "oidc"
  | "api-key"
  | "service-account"
  | "local-credential";

export type IntegrationCredentialStore =
  | "server-vault"
  | "device-keyring"
  | "external-vault"
  | "none";

export type IntegrationStatus =
  | "needs-auth"
  | "needs-config"
  | "connecting"
  | "connected"
  | "error"
  | "disabled"
  | "revoked";

export type IntegrationFieldType =
  | "text"
  | "password"
  | "url"
  | "select"
  | "boolean"
  | "json";

export interface IntegrationFieldOption {
  value: string;
  label: string;
}

export interface IntegrationConfigField {
  name: string;
  label: string;
  fieldType: IntegrationFieldType;
  required: boolean;
  secret: boolean;
  placeholder?: string | null;
  helpText?: string | null;
  options: IntegrationFieldOption[];
}

export interface IntegrationPermission {
  id: string;
  label: string;
  description?: string | null;
  required: boolean;
  sensitive: boolean;
}

export interface IntegrationCapability {
  id: string;
  label: string;
  description?: string | null;
  readOnly: boolean;
  permissionIds: string[];
}

export interface IntegrationOAuthConfig {
  authorizationUrl: string;
  tokenUrl: string;
  userinfoUrl?: string | null;
  revocationUrl?: string | null;
  redirectPath: string;
  defaultScopes: string[];
  pkceRequired: boolean;
  oidc: boolean;
}

export interface IntegrationAuthConfig {
  mode: IntegrationAuthMode;
  credentialStore: IntegrationCredentialStore;
  oauth?: IntegrationOAuthConfig | null;
  configFields: IntegrationConfigField[];
}

export interface IntegrationManifest {
  id: string;
  version: string;
  kind: IntegrationKind;
  displayName: string;
  description: string;
  category: string;
  icon?: string | null;
  auth: IntegrationAuthConfig;
  permissions: IntegrationPermission[];
  capabilities: IntegrationCapability[];
}

export interface IntegrationAccountProfile {
  externalAccountId?: string | null;
  displayName?: string | null;
  email?: string | null;
  avatarUrl?: string | null;
  details?: Record<string, unknown> | null;
}

export interface IntegrationCredentialRef {
  id?: string;
  type?: string;
  store?: IntegrationCredentialStore;
  keyId?: string | null;
  keyVersion?: string | null;
  grantedScopes?: string[];
  createdAt: string;
  expiresAt?: string | null;
  lastUsedAt?: string | null;
  revokedAt?: string | null;
}

export type ConnectorAuthMethod =
  | "authorization_code_pkce"
  | "open_id_connect"
  | "device_code"
  | "app_installation"
  | "api_key"
  | "personal_access_token"
  | "service_account"
  | "client_credentials"
  | "local_credential"
  | "external_vault";

export type InstallationLifecycleStatus =
  | "pending"
  | "active"
  | "reauthorization_required"
  | "disconnecting"
  | "revoked"
  | "deleted";

export type InstallationHealthStatus =
  | "unknown"
  | "healthy"
  | "degraded"
  | "provider_unavailable"
  | "credentials_expired"
  | "permissions_insufficient"
  | "invalid_credentials"
  | "sync_error";

export interface ConnectorDefinition {
  id: string;
  version: string;
  displayName: string;
  description: string;
  category: string;
  icon?: string | null;
  authMethods: ConnectorAuthMethod[];
  permissions: IntegrationPermission[];
  capabilities: Array<IntegrationCapability & { scopes?: string[] }>;
  allowedHosts: string[];
  enabled: boolean;
  certificationStatus: "uncertified" | "testing" | "certified" | "suspended";
  publicConfigSchema?: Record<string, unknown> | null;
}

export interface InstallationOwner {
  type: "user" | "team" | "organization";
  userId?: string | null;
  teamId?: string | null;
}

export interface IntegrationInstallation {
  id: string;
  organizationId: string;
  providerId: string;
  providerVersion: string;
  owner: InstallationOwner;
  label?: string | null;
  lifecycleStatus: InstallationLifecycleStatus;
  healthStatus: InstallationHealthStatus;
  enabled: boolean;
  publicConfig: Record<string, unknown>;
  account?: IntegrationAccountProfile | null;
  credential?: IntegrationCredentialRef | null;
  version: number;
  connectedAt?: string | null;
  revokedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CredentialInstallationInput {
  owner: InstallationOwner;
  credentialType: "api_key" | "personal_access_token" | "service_account" | "external_vault";
  secret: string | Record<string, unknown>;
  label?: string | null;
  publicConfig?: Record<string, unknown>;
  capabilityIds?: string[];
  grantedScopes?: string[];
  expiresAt?: string | null;
}

export interface AuthorizationAttemptInput {
  owner: InstallationOwner;
  method: Extract<
    ConnectorAuthMethod,
    "authorization_code_pkce" | "open_id_connect" | "app_installation"
  >;
  capabilityIds?: string[];
}

export interface AuthorizationAttemptResponse {
  attemptId: string;
  interaction: {
    type: "browser_redirect";
    url: string;
  };
  expiresAt: string;
}

export type AuthorizationAttemptStatus =
  | "pending"
  | "processing"
  | "authorized"
  | "denied"
  | "failed"
  | "expired"
  | "cancelled";

export interface AuthorizationAttemptView {
  id: string;
  providerId: string;
  authMethod: ConnectorAuthMethod;
  owner: InstallationOwner;
  capabilityIds: string[];
  requestedScopes: string[];
  status: AuthorizationAttemptStatus;
  errorCode?: string | null;
  installationId?: string | null;
  expiresAt: string;
  completedAt?: string | null;
  createdAt: string;
}

export interface IntegrationConnection {
  id: string;
  organizationId: string;
  ownerUserId?: string | null;
  providerId: string;
  manifestVersion: string;
  status: IntegrationStatus;
  enabled: boolean;
  grantedScopes: string[];
  account: IntegrationAccountProfile;
  credential?: IntegrationCredentialRef | null;
  publicConfig: Record<string, unknown>;
  lastCheckedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface LegacyPluginField {
  name: string;
  label: string;
  placeholder: string;
  type: "text" | "password";
}

export interface LegacyPluginLike {
  id: string;
  name: string;
  description: string;
  icon?: string | null;
  category: string;
  authType: "none" | "api_key" | "oauth";
  configFields: LegacyPluginField[];
  subServices?: Array<{ id: string; label: string; enabled: boolean; icon: string }>;
}
