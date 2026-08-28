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

const mnemonic = loadMnemonic();
const buyerW = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
  prefix: "osmo",
  hdPaths: [stringToPath("m/44'/118'/2'/0/0")],
});
const adminW = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "osmo" });
const [buyer] = await buyerW.getAccounts();
const [admin] = await adminW.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, buyerW);
const micro = Number(process.env.FUND_OSMO ?? "1.2") * 1_000_000;
await client.sendTokens(buyer.address, admin.address, coins(micro, "uosmo"), fee, "buyer tops admin");
console.log(`Sent ${micro / 1e6} OSMO to admin`);
const bal = await client.getBalance(admin.address, "uosmo");
console.log("Admin balance", Number(bal.amount) / 1e6, "OSMO");
