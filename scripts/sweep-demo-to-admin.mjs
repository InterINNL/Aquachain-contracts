#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadMnemonic } from "./load-mnemonic.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);
const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");
const { stringToPath } = require("@cosmjs/crypto");
const { coins } = require("@cosmjs/amino");

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const fee = { amount: [{ denom: "uosmo", amount: "80000" }], gas: "1500000" };
const KEEP = Number(process.env.KEEP_OSMO ?? "0.05") * 1_000_000;

const mnemonic = loadMnemonic();
const adminW = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "osmo" });
const [admin] = await adminW.getAccounts();

for (const index of [1, 2]) {
  const w = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "osmo",
    hdPaths: [stringToPath(`m/44'/118'/${index}'/0/0`)],
  });
  const [acc] = await w.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, w);
  const bal = await client.getBalance(acc.address, "uosmo");
  const send = Number(bal.amount) - KEEP - 80_000;
  if (send <= 0) {
    console.log(`skip ${acc.address.slice(0, 12)} balance too low`);
    continue;
  }
  await client.sendTokens(acc.address, admin.address, coins(send, "uosmo"), fee, "sweep to admin");
  console.log(`swept ${(send / 1e6).toFixed(3)} OSMO from account ${index}`);
}

const adminClient = await SigningCosmWasmClient.connect(RPC);
const final = await adminClient.getBalance(admin.address, "uosmo");
console.log("Admin total", (Number(final.amount) / 1e6).toFixed(3), "OSMO");
