import { describe, it, expect } from "vitest";
import { defaultCatalog, eurosToMicros, trustedPaymentUrl } from "./model";
describe("commercial boundaries", () => {
  it("keeps inference separate from every software offer", () => { expect(defaultCatalog.inferenceIncluded).toBe(false); expect(defaultCatalog.plans.find(p => p.id === "business")?.amountCents).toBe(2900); });
  it("converts money without float rounding or exponent input", () => { expect(eurosToMicros("12,34")).toBe(12_340_000); expect(eurosToMicros("0")).toBe(0); for (const input of ["-1", "1e3", "0.001", "NaN", "1000001", "1.2.3"]) expect(() => eurosToMicros(input)).toThrow(); });
  it("opens only payment-provider pages", () => { expect(trustedPaymentUrl("https://checkout.stripe.com/c/pay/test")).toContain("stripe.com"); for (const url of ["javascript:alert(1)","https://checkout.stripe.com.evil.test/", "https://user:password@billing.stripe.com/", "http://billing.stripe.com/", "https://checkout.stripe.com:8443/pay"]) expect(() => trustedPaymentUrl(url)).toThrow(); });
});
