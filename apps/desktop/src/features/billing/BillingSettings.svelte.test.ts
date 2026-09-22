import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import BillingSettings from "../settings/pages/BillingSettings.svelte";
import { defaultCatalog } from "./model";
const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock("../../lib/api/billing", () => ({ billingRequest: request }));

const account = () => ({ account: { plan:"community", status:"inactive", seats:1, validUntil:null, balanceMicros:8_000_000, reservedMicros:2_000_000, monthlyLimitMicros:10_000_000, perRequestLimitMicros:1_000_000, monthSpentMicros:0 }, entitlements:{managedSync:false},canManage:true,checkoutAvailable:false,computeAvailable:false,pendingCheckoutId:null,ledger:[] });
beforeEach(() => {
  request.mockReset();
  request.mockImplementation(async (_method:string,path:string) => path === "/billing/catalog" ? {...defaultCatalog,checkoutAvailable:false} : path === "/billing/account" ? account() : []);
});
describe("billing boundaries in settings", () => {
  it("shows no usable checkout or paid key creation when server is unconfigured", async () => {
    const view=render(BillingSettings,{authenticated:true,language:"fr"});
    await waitFor(()=>expect(view.getByText("Créer une clé")).toBeDisabled());
    for (const button of view.getAllByRole("button").filter(b=>b.textContent?.includes("HT de crédit"))) expect(button).toBeDisabled();
    expect(request.mock.calls.every(([method])=>method==="GET")).toBe(true);
  });
  it("rejects a per-call budget above monthly budget before a server write", async () => {
    const view=render(BillingSettings,{authenticated:true,language:"fr"});
    await waitFor(()=>expect(view.getByText("Enregistrer les budgets")).not.toBeDisabled());
    await fireEvent.input(view.getByLabelText("Plafond mensuel (€)"),{target:{value:"1"}});
    await fireEvent.input(view.getByLabelText("Plafond par requête (€)"),{target:{value:"2"}});
    await fireEvent.click(view.getByText("Enregistrer les budgets"));
    await waitFor(()=>expect(view.getByText("Le plafond par requête doit être inférieur au plafond mensuel.")).toBeInTheDocument());
    expect(request.mock.calls.some(([method])=>method==="PUT")).toBe(false);
  });
  it("does not fabricate account credit after a server error", async () => {
    request.mockRejectedValue(new Error("Compte indisponible"));
    const view=render(BillingSettings,{authenticated:true,language:"fr"});
    await waitFor(()=>expect(view.getByText("Compte indisponible")).toBeInTheDocument());
    expect(view.queryByText("Créer une clé")).toBeNull();
  });
});
