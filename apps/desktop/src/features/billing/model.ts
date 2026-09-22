import catalog from "../../../../../packages/contracts/src/commercial-catalog.json";
export type PlanId = "community" | "cloud" | "business" | "enterprise";
export interface CommercialOffer { id: string; name: string; amountCents: number; interval: string; taxBehavior: string; perSeat: boolean; commercialUse: boolean; selfServe: boolean; startingAt?: boolean; features: string[] }
export interface CommercialCatalog { version: string; currency: string; licenseModel: string; pricingStatus: string; inferenceIncluded: boolean; plans: CommercialOffer[]; checkoutAvailable?: boolean; computeAvailable?: boolean; salesEmail?: string | null }
export const defaultCatalog: CommercialCatalog = catalog;
export interface BillingSnapshot {
  account: { plan: PlanId; status: string; seats: number; validUntil: string | null; cancelAtPeriodEnd: boolean; balanceMicros: number; reservedMicros: number; monthlyLimitMicros: number; perRequestLimitMicros: number; monthSpentMicros: number };
  entitlements: { commercialUse: boolean; managedSync: boolean; teamAdministration: boolean; dataExport: boolean; localModels: boolean; byok: boolean };
  canManage: boolean; checkoutAvailable: boolean; computeAvailable: boolean; portalAvailable?: boolean; pendingCheckoutId: string | null;
  ledger: { id: string; kind: string; amountMicros: number; createdAt: string }[];
}
export interface ComputeKey { id: string; name: string; prefix: string; createdAt: string }
export function eurosToMicros(value: string): number {
  const text = value.trim().replace(",", ".");
  if (!/^\d{1,7}(\.\d{1,2})?$/.test(text)) throw new Error("Saisissez un montant positif avec au plus deux décimales.");
  const [whole, fraction = ""] = text.split(".");
  const amount = Number(whole) * 1_000_000 + Number(fraction.padEnd(2, "0")) * 10_000;
  if (!Number.isSafeInteger(amount) || amount > 1_000_000_000_000) throw new Error("Montant trop élevé.");
  return amount;
}
export function formatMicros(value: number, language = "fr"): string {
  return new Intl.NumberFormat(language === "fr" ? "fr-FR" : "en-GB", { style: "currency", currency: "EUR", minimumFractionDigits: 2, maximumFractionDigits: value !== 0 && Math.abs(value) < 10_000 ? 6 : 2 }).format(value / 1_000_000);
}
export function trustedPaymentUrl(value: string): string {
  const url = new URL(value);
  if (url.protocol !== "https:" || url.port || !["checkout.stripe.com", "billing.stripe.com"].includes(url.hostname) || url.username || url.password) throw new Error("Adresse de paiement non autorisée.");
  return url.href;
}
