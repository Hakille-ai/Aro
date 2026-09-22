import { invoke } from "@tauri-apps/api/core";
import { webFetch } from "./transport";
export async function billingRequest<T>(method: string, path: string, body?: unknown): Promise<T> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) return invoke<T>("billing_request", { method, path, body: body ?? null });
  return webFetch<T>(method, path, body, path !== "/billing/catalog");
}
