import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = resolve(dirname(fileURLToPath(import.meta.url)));
const CONTRACTS_ROOT = resolve(SCRIPTS_DIR, "..");
const DEPLOY_SECRETS = resolve(CONTRACTS_ROOT, "../../.secrets/osmosis-deploy.env");
const LEGACY_ENV = resolve(CONTRACTS_ROOT, ".env");

function parseMnemonicLine(line) {
  const raw = line.slice("MNEMONIC=".length).trim();
  if (
    (raw.startsWith("'") && raw.endsWith("'")) ||
    (raw.startsWith('"') && raw.endsWith('"'))
  ) {
    return raw.slice(1, -1).trim();
  }
  return raw;
}

function readMnemonicFromFile(path) {
  if (!existsSync(path)) return null;
  const line = readFileSync(path, "utf8")
    .split(/\r?\n/)
    .find((row) => row.startsWith("MNEMONIC="));
  return line ? parseMnemonicLine(line) : null;
}

/** Deploy admin mnemonic: MNEMONIC env, then .secrets/osmosis-deploy.env, then contracts/.env. */
export function loadMnemonic() {
  const fromEnv = process.env.MNEMONIC?.trim();
  if (fromEnv) return fromEnv;
  for (const path of [DEPLOY_SECRETS, LEGACY_ENV]) {
    const mnemonic = readMnemonicFromFile(path);
    if (mnemonic) return mnemonic;
  }
  throw new Error(
    "Set MNEMONIC in .secrets/osmosis-deploy.env, export MNEMONIC, or legacy contracts/.env",
  );
}
