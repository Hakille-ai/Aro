import { describe, it, expect } from "vitest";
import {
  getBrandLogo,
  brandTileCss,
  brandTileInner,
  isValidPluginSlug,
  validateLogoUpload,
} from "./brandLogos";

describe("brandLogos", () => {
  it("maps GitHub to the dark tile with vendored SVG mark", () => {
    const brand = getBrandLogo("github-developer");
    expect(brand.bg).toBe("#181717");
    expect(brand.svg).toContain("<svg");
    expect(brand.svg).toContain("<path");
  });

  it("maps Google Workspace / Drive to the Drive mark", () => {
    const brand = getBrandLogo("google-workspace");
    expect(brand.fg).toBe("#1A73E8");
    expect(brand.svg).toContain("<path");
  });

  it("maps Slack and OpenAI to geometric marks (no emoji)", () => {
    expect(getBrandLogo("slack-workspace").svg).toContain("<svg");
    expect(getBrandLogo("openai-ecosystem").svg).toContain("<svg");
  });

  it("matches case-insensitively on server names", () => {
    expect(getBrandLogo("unknown-id", { serverName: "github-mcp" }).bg).toBe("#181717");
    expect(getBrandLogo("x", { serverName: "google-workspace-mcp" }).fg).toBe("#1A73E8");
  });

  it("honors explicit logo emoji + brandColor overrides", () => {
    const brand = getBrandLogo("my-plugin", { logo: "🚀", brandColor: "#ff0000" });
    expect(brand.svg).toBe("");
    expect(brand.emoji).toBe("🚀");
    expect(brand.bg).toBe("#ff0000");
  });

  it("falls back to generic tile for unknown plugins", () => {
    const brand = getBrandLogo("some-random-plugin");
    expect(brand.id).toBe("generic");
    expect(brand.emoji).toBe("🧩");
  });

  it("flags file logos for lazy loading without breaking tiles", () => {
    const brand = getBrandLogo("my-custom-plugin", {
      logo: "file:logo.png",
      logoKind: "file",
      brandColor: "#0A84FF",
    });
    expect(brand.filePluginId).toBe("my-custom-plugin");
    expect(brand.bg).toBe("#0A84FF");
    expect(brandTileInner(brand)).toContain("🧩");
  });

  it("validates plugin slugs like the spec (§5.5)", () => {
    expect(isValidPluginSlug("mon-plugin-assistant")).toBe(true);
    expect(isValidPluginSlug("acme.tools")).toBe(true);
    expect(isValidPluginSlug("")).toBe(false);
    expect(isValidPluginSlug("My-Plugin")).toBe(false);
    expect(isValidPluginSlug("../../evil")).toBe(false);
    expect(isValidPluginSlug("has--double")).toBe(false);
    expect(isValidPluginSlug("x".repeat(65))).toBe(false);
  });

  it("validates logo uploads (type + size)", () => {
    const png = new File(["x".repeat(10)], "logo.png", { type: "image/png" });
    expect(validateLogoUpload(png).ok).toBe(true);
    const gif = new File(["x"], "logo.gif", { type: "image/gif" });
    expect(validateLogoUpload(gif).ok).toBe(false);
    const big = new File([new Uint8Array(3 * 1024 * 1024)], "big.png", { type: "image/png" });
    const res = validateLogoUpload(big);
    expect(res.ok).toBe(false);
    expect(res.error).toContain("2 Mo");
    const hd = new File([new Uint8Array(600 * 1024)], "hd.png", { type: "image/png" });
    expect(validateLogoUpload(hd).ok).toBe(true);
  });

  it("renders tile css + inner html without throwing", () => {
    const brand = getBrandLogo("github-developer");
    expect(brandTileCss(brand, 38)).toContain("width:38px");
    expect(brandTileInner(brand)).toContain("<svg");
    const generic = getBrandLogo("unknown");
    expect(brandTileInner(generic)).toContain("🧩");
  });
});
