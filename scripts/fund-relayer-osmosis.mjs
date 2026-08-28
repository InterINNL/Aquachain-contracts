#!/usr/bin/env node
/**
 * Send test OSMO from the deploy admin wallet to the gateway relayer wallet.
 *
 *   cd aquachain/contracts/scripts
 *   RELAYER_ADDRESS=osmo1… node fund-relayer-osmosis.mjs
 */
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);
const { SigningStargateClient } = require("@cosmjs/stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");
const { coins } = require("@cosmjs/amino");

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const fee = { amount: [{ denom: "uosmo", amount: "80000" }], gas: "200000" };

function loadMnemonic() {
  const line = readFileSync(resolve(ROOT, ".env"), "utf8")
    .split(/\r?\n/)
    .find((row) => row.startsWith("MNEMONIC="));
  if (!line) throw new Error("MNEMONIC missing in contracts/.env");
  const raw = line.slice("MNEMONIC=".length).trim();
  if (raw.startsWith("'") && raw.endsWith("'")) return raw.slice(1, -1);
  return raw;
}

const relayer =
  process.env.RELAYER_ADDRESS?.trim() ??
  readFileSync(resolve(ROOT, "../../.secrets/osmosis-relayer.env"), "utf8")
    .split(/\r?\n/)
    .find((row) => row.startsWith("RELAYER_ADDRESS="))
    ?.slice("RELAYER_ADDRESS=".length)
    .trim();

if (!relayer?.startsWith("osmo1")) {
  console.error("Set RELAYER_ADDRESS to an osmo1… address");
  process.exit(1);
}

const osmo = Number(process.env.FUND_OSMO ?? "2");
const micro = Math.round(osmo * 1_000_000);
const mnemonic = loadMnemonic();
const adminW = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
  prefix: "osmo",
});
const [admin] = await adminW.getAccounts();
const client = await SigningStargateClient.connectWithSigner(RPC, adminW);
const before = await client.getBalance(relayer, "uosmo");
await client.sendTokens(
  admin.address,
  relayer,
  coins(micro, "uosmo"),
  fee,
  "fund gateway relayer",
);
const after = await client.getBalance(relayer, "uosmo");
console.log(`Sent ${osmo} OSMO from ${admin.address} to ${relayer}`);
console.log(
  "Relayer balance:",
  Number(after.amount) / 1e6,
  "OSMO (was",
  Number(before.amount) / 1e6,
  ")",
);
