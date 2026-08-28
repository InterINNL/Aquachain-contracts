import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadMnemonic } from "./load-mnemonic.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");

/** Osmosis deploy wallet from PRIVATE_KEY, MNEMONIC env, or .secrets/osmosis-deploy.env. */
export async function loadWallet(options = {}) {
  const { prefix = "osmo", hdPaths } = options;
  const privateKey = process.env.PRIVATE_KEY?.trim().replace(/^0x/i, "");
  if (privateKey && /^[0-9a-fA-F]{64}$/.test(privateKey)) {
    return DirectSecp256k1Wallet.fromKey(
      Uint8Array.from(Buffer.from(privateKey, "hex")),
      prefix,
    );
  }
  try {
    const mnemonic = loadMnemonic();
    const walletOpts = { prefix };
    if (hdPaths) walletOpts.hdPaths = hdPaths;
    return DirectSecp256k1HdWallet.fromMnemonic(mnemonic, walletOpts);
  } catch (err) {
    console.error(err instanceof Error ? err.message : err);
    console.error("Or set PRIVATE_KEY (32-byte hex) for an osmo1 account.");
    process.exit(1);
  }
}
