import {
  findSeedManifest,
  isLikelySecretFieldName,
  manifestFromLegacyPlugin,
  manifestSecretFieldNames,
  requiresRedirect,
} from ".";

declare const describe: (name: string, fn: () => void) => void;
declare const it: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => {
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toMatchObject(expected: unknown): void;
};

describe("integration registry", () => {
  it("detects declared OAuth manifests", () => {
    const github = findSeedManifest("github");

    expect(Boolean(github)).toBe(true);
    expect(requiresRedirect(github!)).toBe(true);
    expect(manifestSecretFieldNames(github!)).toEqual([]);
  });

  it("classifies legacy password fields as secret", () => {
    const manifest = manifestFromLegacyPlugin({
      id: "plg-example",
      name: "Example",
      description: "Legacy plugin",
      icon: "plug",
      category: "Development",
      authType: "api_key",
      configFields: [
        { name: "workspace", label: "Workspace", placeholder: "team", type: "text" },
        { name: "client_secret", label: "Secret", placeholder: "secret", type: "password" },
      ],
    });

    expect(manifest.id).toBe("example");
    expect(manifest.auth.mode).toBe("api-key");
    expect(manifest.auth.credentialStore).toBe("server-vault");
    expect(manifestSecretFieldNames(manifest)).toEqual(["client_secret"]);
  });

  it("keeps common credential names out of public config", () => {
    expect(isLikelySecretFieldName("authorization")).toBe(true);
    expect(isLikelySecretFieldName("private_key")).toBe(true);
    expect(isLikelySecretFieldName("workspace_slug")).toBe(false);
  });
});
