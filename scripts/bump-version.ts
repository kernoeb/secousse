#!/usr/bin/env bun
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const newVersion = process.argv[2];
if (!newVersion || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(newVersion)) {
  console.error("Usage: bun run bump <semver>");
  console.error("Example: bun run bump 0.2.0");
  process.exit(1);
}

const root = resolve(import.meta.dir, "..");

const pkgPath = resolve(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
pkg.version = newVersion;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const tauriPath = resolve(root, "src-tauri/tauri.conf.json");
const tauri = JSON.parse(readFileSync(tauriPath, "utf8"));
tauri.version = newVersion;
writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

const cargoPath = resolve(root, "src-tauri/Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const versionRegex = /^(version\s*=\s*)"[^"]*"/m;
if (!versionRegex.test(cargo)) {
  console.error("Failed to bump Cargo.toml — version line not found");
  process.exit(1);
}
writeFileSync(cargoPath, cargo.replace(versionRegex, `$1"${newVersion}"`));

console.log(`Bumped to ${newVersion}`);
console.log("");
console.log("Next steps:");
console.log("  cd src-tauri && cargo check    # refresh Cargo.lock");
console.log(`  git commit -am "chore: bump version to ${newVersion}"`);
console.log(`  git tag v${newVersion}`);
console.log("  git push && git push --tags");
