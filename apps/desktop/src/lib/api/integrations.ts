import { invoke } from "@tauri-apps/api/core";
import type {
  AuthorizationAttemptInput,
  AuthorizationAttemptResponse,
  AuthorizationAttemptView,
  ConnectorDefinition,
  CredentialInstallationInput,
  IntegrationInstallation,
} from "../integrations/types";
import { isTauri, isWeb, webFetch, webToken } from "./transport";

export async function listIntegrationCatalog(): Promise<ConnectorDefinition[]> {
  if (isTauri()) return invoke("integration_catalog_v3_list");
  if (isWeb() && webToken()) return webFetch("GET", "/integration-catalog");
  return [];
}

export async function listIntegrationInstallations(): Promise<IntegrationInstallation[]> {
  if (isTauri()) return invoke("integration_installations_v3_list");
  if (isWeb() && webToken()) return webFetch("GET", "/integration-installations");
  return [];
}

export async function createIntegrationCredential(
  providerId: string,
  request: CredentialInstallationInput,
  idempotencyKey = crypto.randomUUID(),
): Promise<IntegrationInstallation> {
  if (isTauri()) {
    return invoke("integration_credential_v3_create", { providerId, idempotencyKey, request });
  }
  if (isWeb() && webToken()) {
    throw new Error("Credential entry is disabled in the web client until HttpOnly BFF sessions are available.");
  }
  throw new Error("A signed-in desktop session is required.");
}

export async function startIntegrationAuthorization(
  providerId: string,
  request: AuthorizationAttemptInput,
  idempotencyKey = crypto.randomUUID(),
): Promise<AuthorizationAttemptResponse> {
  if (isTauri()) {
    return invoke("integration_authorization_v3_create", { providerId, idempotencyKey, request });
  }
  throw new Error("OAuth connections currently require the signed desktop application.");
}

export async function getIntegrationAuthorizationAttempt(
  attemptId: string,
): Promise<AuthorizationAttemptView> {
  if (isTauri()) return invoke("integration_authorization_v3_get", { attemptId });
  return webFetch("GET", `/integration-authorization-attempts/${attemptId}`);
}

export async function requestIntegrationHealthCheck(installationId: string): Promise<{ jobId: string; status: string }> {
  if (isTauri()) return invoke("integration_installation_v3_action", { installationId, action: "health-check" });
  return webFetch("POST", `/integration-installations/${installationId}/health-check`, {});
}

export async function disconnectIntegration(installationId: string): Promise<{ jobId: string; status: string }> {
  if (isTauri()) return invoke("integration_installation_v3_action", { installationId, action: "disconnect" });
  return webFetch("POST", `/integration-installations/${installationId}/disconnect`, {});
}
