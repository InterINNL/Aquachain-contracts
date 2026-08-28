#!/usr/bin/env node
/**
 * Deploy CosmWasm artifact to Osmosis testnet (osmo-test-5).
 *
 * Usage:
 *   MNEMONIC='…' node deploy-osmosis.mjs [path-to.wasm]
 *   PRIVATE_KEY='…hex…' node deploy-osmosis.mjs [path-to.wasm]
 *
 * Optional: LABEL=water-well-initiative
 *
 * Requires frontend node_modules (@cosmjs/*) under repo-root www/.
 */
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");

const RPC = process.env.RPC ?? "https://rpc.testnet.osmosis.zone";
const CHAIN_ID = process.env.CHAIN_ID ?? "osmo-test-5";
const DENOM = process.env.DENOM ?? "uosmo";
const LABEL = process.env.LABEL ?? "citizen-science-registry";

// Explicit fees: CosmJS 0.39 "auto" + GasPrice breaks on duplicate @cosmjs/stargate copies.
const uploadFee = {
  amount: [{ denom: DENOM, amount: "2500000" }],
  gas: "10000000",
};
const instantiateFee = {
  amount: [{ denom: DENOM, amount: "500000" }],
  gas: "2000000",
};

const defaultWasm = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../target/wasm32-unknown-unknown/release/citizen_science_registry.wasm",
);
const wasmPath = resolve(process.argv[2] ?? defaultWasm);

async function loadWallet() {
  const mnemonic = process.env.MNEMONIC?.trim();
  if (mnemonic) {
    return DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "osmo" });
  }
  const privateKey = process.env.PRIVATE_KEY?.trim().replace(/^0x/i, "");
  if (privateKey && /^[0-9a-fA-F]{64}$/.test(privateKey)) {
    return DirectSecp256k1Wallet.fromKey(
      Uint8Array.from(Buffer.from(privateKey, "hex")),
      "osmo",
    );
  }
  console.error(
    "Set MNEMONIC or PRIVATE_KEY (32-byte hex) for an osmo1… account.",
  );
  process.exit(1);
}

const wasm = readFileSync(wasmPath);
console.log(`Chain  ${CHAIN_ID}`);
console.log(`RPC    ${RPC}`);
console.log(`WASM   ${wasmPath} (${wasm.length} bytes)`);

const wallet = await loadWallet();
const [account] = await wallet.getAccounts();
console.log(`Signer ${account.address}`);

const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);

console.log("Uploading…");
const upload = await client.upload(account.address, wasm, uploadFee);
console.log(`codeId  ${upload.codeId}`);
console.log(`tx      ${upload.transactionHash}`);

console.log("Instantiating…");
const initMsg = process.env.INSTANTIATE_MSG
  ? JSON.parse(process.env.INSTANTIATE_MSG)
  : { denom: DENOM };
const inst = await client.instantiate(
  account.address,
  upload.codeId,
  initMsg,
  LABEL,
  instantiateFee,
  { admin: account.address },
);
console.log(`contract ${inst.contractAddress}`);
console.log(`tx       ${inst.transactionHash}`);
console.log(
  `explorer https://www.mintscan.io/osmosis-testnet/tx/${inst.transactionHash}`,
);
