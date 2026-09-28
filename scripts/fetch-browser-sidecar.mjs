// Downloads the Chrome-for-Testing headless-shell sidecar for the current
// platform into apps/desktop/src-tauri/binaries/ (Tauri `externalBin`).
//
// Usage:
//   node scripts/fetch-browser-sidecar.mjs [--version <rev>] [--force]
//   npm run browser:sidecar
//
// Resolution order at runtime (crates/aro-browser/src/config.rs):
//   1. ARO_CHROME_EXECUTABLE env override
//   2. aro-chromium sidecar next to the app binary (this script)
//   3. system Chrome / Edge / Chromium channel

import { createWriteStream, existsSync, mkdirSync, rmSync } from "node:fs";
import { get } from "node:https";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const binariesDir = join(root, "apps", "desktop", "src-tauri", "binaries");

const args = process.argv.slice(2);
const force = args.includes("--force");
const versionFlag = args.indexOf("--version");
const pinnedRevision = versionFlag >= 0 ? args[versionFlag + 1] : null;

function platformKey() {
  if (process.platform === "win32") return { cft: "win64", triple: "x86_64-pc-windows-msvc", ext: ".exe" };
  if (process.platform === "darwin") {
    return process.arch === "arm64"
      ? { cft: "mac-arm64", triple: "aarch64-apple-darwin", ext: "" }
      : { cft: "mac-x64", triple: "x86_64-apple-darwin", ext: "" };
  }
  return { cft: "linux64", triple: "x86_64-unknown-linux-gnu", ext: "" };
}

function fetchJson(url) {
  return new Promise((resolve, reject) => {
    get(url, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        fetchJson(res.headers.location).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        return;
      }
      let body = "";
      res.on("data", (chunk) => (body += chunk));
      res.on("end", () => {
        try {
          resolve(JSON.parse(body));
        } catch (err) {
          reject(err);
        }
      });
    }).on("error", reject);
  });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    get(url, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        file.close();
        download(res.headers.location, dest).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        return;
      }
      res.pipe(file);
      file.on("finish", () => file.close(resolve));
    }).on("error", (err) => {
      file.close();
      reject(err);
    });
  });
}

async function main() {
  const { cft, triple, ext } = platformKey();
  const outName = `aro-chromium-${triple}${ext}`;
  const outPath = join(binariesDir, outName);
  if (existsSync(outPath) && !force) {
    console.log(`[browser-sidecar] already present: ${outName} (use --force to re-download)`);
    return;
  }
  console.log("[browser-sidecar] resolving Chrome-for-Testing headless-shell…");
  const index = await fetchJson(
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json"
  );
  const channel = index.channels?.Stable;
  const revision = pinnedRevision || channel?.revision;
  const downloads = channel?.downloads?.["chrome-headless-shell"];
  const entry = downloads?.find((d) => d.platform === cft);
  if (!revision || !entry) throw new Error(`no chrome-headless-shell build for platform ${cft}`);
  console.log(`[browser-sidecar] revision ${revision} (${cft})`);
  mkdirSync(binariesDir, { recursive: true });
  const tmpZip = join(tmpdir(), `aro-headless-shell-${Date.now()}.zip`);
  await download(entry.url, tmpZip);
  const tmpDir = join(tmpdir(), `aro-headless-shell-${Date.now()}`);
  mkdirSync(tmpDir, { recursive: true });
  if (process.platform === "win32") {
    execSync(`powershell -NoProfile -Command "Expand-Archive -Path '${tmpZip}' -DestinationPath '${tmpDir}' -Force"`, { stdio: "inherit" });
  } else {
    execSync(`unzip -o -q "${tmpZip}" -d "${tmpDir}"`, { stdio: "inherit" });
  }
  const candidates = [
    join(tmpDir, `chrome-headless-shell-${cft}`, `chrome-headless-shell${ext}`),
    join(tmpDir, `chrome-headless-shell-${cft}`, "chrome-headless-shell"),
    join(tmpDir, `headless-shell-${cft}`, `headless-shell${ext}`),
  ];
  const found = candidates.find((p) => existsSync(p));
  if (!found) throw new Error("headless-shell binary not found in archive");
  execSync(process.platform === "win32" ? `copy /Y "${found}" "${outPath}"` : `cp -f "${found}" "${outPath}"`, { stdio: "inherit" });
  if (process.platform !== "win32") execSync(`chmod +x "${outPath}"`);
  rmSync(tmpZip, { force: true });
  rmSync(tmpDir, { recursive: true, force: true });
  console.log(`[browser-sidecar] installed: ${outPath}`);
  console.log("[browser-sidecar] Tauri bundles it via bundle.externalBin (binaries/aro-chromium).");
}

main().catch((err) => {
  console.error("[browser-sidecar] failed:", err.message);
  process.exit(1);
});
