export type CommercialPlan = 'community' | 'cloud' | 'business' | 'enterprise';
export interface CommercialEntitlements {
  commercialUse: boolean; managedSync: boolean; teamAdministration: boolean;
  localModels: boolean; byok: boolean; dataExport: boolean;
}
export interface BillingAccount {
  plan: CommercialPlan; status: string; seats: number; validUntil: string | null;
  cancelAtPeriodEnd: boolean; balanceMicros: number; reservedMicros: number;
  monthlyLimitMicros: number; perRequestLimitMicros: number; monthSpentMicros: number;
}
export interface BillingSnapshot {
  account: BillingAccount; entitlements: CommercialEntitlements; canManage: boolean;
  checkoutAvailable: boolean; computeAvailable: boolean; portalAvailable: boolean;
  pendingCheckoutId: string | null;
  ledger: {id: string; kind: 'credit' | 'charge' | 'refund' | 'adjustment'; amountMicros: number; createdAt: string}[];
}
export interface ComputeKey {id: string; name: string; prefix: string; createdAt: string}
export interface ComputeCompletionRequest {
  model: string; messages: {role: 'system' | 'user' | 'assistant'; content: string}[];
  max_tokens?: number; temperature?: number; response_format?: {type: 'text' | 'json_object'};
}
export interface ComputeCompletion {
  id: string; object: 'chat.completion'; created: number; model: string;
  choices: {index: number; message: {role: 'assistant'; content: string}; finish_reason: string}[];
  usage: {prompt_tokens: number; completion_tokens: number; total_tokens?: number};
  aroBilling: {chargedMicros: number; currency: 'EUR'; requestId: string};
}
