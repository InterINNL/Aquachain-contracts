#!/usr/bin/env node
/** Cast demo votes on Local DAO from volunteer + buyer HD paths. */
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

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const CONTRACT =
  process.env.CONTRACT ??
  JSON.parse(
    readFileSync(
      resolve(
        dirname(fileURLToPath(import.meta.url)),
        "deployed-addresses.json",
      ),
      "utf8",
    ),
  ).LocalDaoContractAddress;
const fee = { amount: [{ denom: "uosmo", amount: "80000" }], gas: "1500000" };

const mnemonic = loadMnemonic();
for (const [label, index] of [
  ["volunteer", 1],
  ["buyer", 2],
]) {
  const w = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "osmo",
    hdPaths: [stringToPath(`m/44'/118'/${index}'/0/0`)],
  });
  const [acc] = await w.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, w);
  for (const pid of [1, 2]) {
    try {
      await client.execute(
        acc.address,
        CONTRACT,
        { vote: { proposal_id: pid, vote: { yes: {} } } },
        fee,
        `${label} yes ${pid}`,
      );
      console.log(`${label} voted yes on proposal ${pid}`);
    } catch (e) {
      console.log(`${label} proposal ${pid}:`, e.message?.slice(0, 80));
    }
  }
}
