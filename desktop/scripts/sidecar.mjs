// The tazuna binary the app carries (tauri.conf.json bundle.externalBin): built from this repository and copied to
// src-tauri/binaries/tazuna-<target triple>, the name Tauri looks for. Runs before every `tauri build`.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const desktop = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(desktop, "..");
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)/m)[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const exe = triple.includes("windows") ? ".exe" : "";
const args = ["build", "--release", "--manifest-path", join(root, "Cargo.toml")];
if (triple !== host) args.push("--target", triple);
execFileSync("cargo", args, { stdio: "inherit" });
const built = join(root, "target", ...(triple !== host ? [triple] : []), "release", `tazuna${exe}`);
mkdirSync(join(desktop, "src-tauri", "binaries"), { recursive: true });
copyFileSync(built, join(desktop, "src-tauri", "binaries", `tazuna-${triple}${exe}`));
console.log(`sidecar: tazuna for ${triple}`);
