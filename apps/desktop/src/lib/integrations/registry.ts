import type {
  IntegrationAuthMode,
  IntegrationConfigField,
  IntegrationCredentialStore,
  IntegrationManifest,
  LegacyPluginField,
  LegacyPluginLike,
} from "./types";

const MANIFEST_VERSION = "2026-07-01";

const secretNamePatterns = [
  "token",
  "secret",
  "password",
  "private",
  "credential",
  "authorization",
  "api_key",
  "apikey",
  "access_key",
  "client_secret",
];

export function isLikelySecretFieldName(name: string): boolean {
  const normalized = name.trim().toLowerCase();
  return secretNamePatterns.some((pattern) => normalized.includes(pattern))
    || normalized.endsWith("key");
}

export function requiresRedirect(manifest: IntegrationManifest): boolean {
  return manifest.auth.mode === "oauth2" || manifest.auth.mode === "oidc";
}

export function manifestSecretFieldNames(manifest: IntegrationManifest): string[] {
  return manifest.auth.configFields
    .filter((field) => field.secret)
    .map((field) => field.name);
}

function mapLegacyAuthMode(authType: LegacyPluginLike["authType"]): IntegrationAuthMode {
  if (authType === "oauth") return "oauth2";
  if (authType === "api_key") return "api-key";
  return "none";
}

function credentialStoreForMode(mode: IntegrationAuthMode): IntegrationCredentialStore {
  if (mode === "none") return "none";
  if (mode === "local-credential") return "device-keyring";
  return "server-vault";
}

function mapLegacyField(field: LegacyPluginField): IntegrationConfigField {
  return {
    name: field.name,
    label: field.label,
    fieldType: field.type === "password" ? "password" : "text",
    required: true,
    secret: field.type === "password" || isLikelySecretFieldName(field.name),
    placeholder: field.placeholder,
    helpText: null,
    options: [],
  };
}

export function manifestFromLegacyPlugin(plugin: LegacyPluginLike): IntegrationManifest {
  const mode = mapLegacyAuthMode(plugin.authType);
  return {
    id: plugin.id.replace(/^plg-/, ""),
    version: MANIFEST_VERSION,
    kind: "app",
    displayName: plugin.name,
    description: plugin.description,
    category: plugin.category,
    icon: plugin.icon ?? null,
    auth: {
      mode,
      credentialStore: credentialStoreForMode(mode),
      oauth: null,
      configFields: plugin.configFields.map(mapLegacyField),
    },
    permissions: (plugin.subServices ?? []).map((service) => ({
      id: service.id,
      label: service.label,
      description: null,
      required: service.enabled,
      sensitive: false,
    })),
    capabilities: (plugin.subServices ?? []).map((service) => ({
      id: service.id,
      label: service.label,
      description: null,
      readOnly: true,
      permissionIds: [service.id],
    })),
  };
}

export const seedIntegrationManifests: IntegrationManifest[] = [
  {
    id: "github",
    version: MANIFEST_VERSION,
    kind: "app",
    displayName: "GitHub",
    description: "Connect repositories, issues, pull requests, and code search.",
    category: "Development",
    icon: "github",
    auth: {
      mode: "oauth2",
      credentialStore: "server-vault",
      oauth: {
        authorizationUrl: "https://github.com/login/oauth/authorize",
        tokenUrl: "https://github.com/login/oauth/access_token",
        userinfoUrl: "https://api.github.com/user",
        revocationUrl: null,
        redirectPath: "/integrations/github/oauth/callback",
        defaultScopes: ["read:user", "repo"],
        pkceRequired: true,
        oidc: false,
      },
      configFields: [],
    },
    permissions: [
      {
        id: "github.profile.read",
        label: "Read profile",
        description: "Show the connected GitHub account.",
        required: true,
        sensitive: false,
      },
      {
        id: "github.repo.read",
        label: "Read repositories",
        description: "Search repositories, issues, and pull requests.",
        required: true,
        sensitive: true,
      },
    ],
    capabilities: [
      {
        id: "github.repo.search",
        label: "Search repositories",
        description: "Find repositories, issues, pull requests, and code references.",
        readOnly: true,
        permissionIds: ["github.repo.read"],
      },
    ],
  },
  {
    id: "google-workspace",
    version: MANIFEST_VERSION,
    kind: "app",
    displayName: "Google Workspace",
    description: "Connect Drive, Docs, Sheets, Slides, Calendar, and Keep.",
    category: "Productivity",
    icon: "google",
    auth: {
      mode: "oidc",
      credentialStore: "server-vault",
      oauth: {
        authorizationUrl: "https://accounts.google.com/o/oauth2/v2/auth",
        tokenUrl: "https://oauth2.googleapis.com/token",
        userinfoUrl: "https://openidconnect.googleapis.com/v1/userinfo",
        revocationUrl: "https://oauth2.googleapis.com/revoke",
        redirectPath: "/integrations/google-workspace/oauth/callback",
        defaultScopes: ["openid", "email", "profile", "https://www.googleapis.com/auth/drive.metadata.readonly"],
        pkceRequired: true,
        oidc: true,
      },
      configFields: [],
    },
    permissions: [
      {
        id: "google.profile.read",
        label: "Read profile",
        description: "Show the connected Google account.",
        required: true,
        sensitive: false,
      },
      {
        id: "google.drive.metadata.read",
        label: "Read Drive metadata",
        description: "List accessible Drive documents without downloading file content by default.",
        required: true,
        sensitive: true,
      },
    ],
    capabilities: [
      {
        id: "google.drive.search",
        label: "Search Drive",
        description: "Find Drive files the user authorized.",
        readOnly: true,
        permissionIds: ["google.drive.metadata.read"],
      },
    ],
  },
];

export function findSeedManifest(id: string): IntegrationManifest | undefined {
  return seedIntegrationManifests.find((manifest) => manifest.id === id);
}
