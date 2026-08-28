#!/usr/bin/env node
/**
 * Fresh deploy of all eight AquaChain contracts (fa75 admin) + update prod env.
 *
 *   set -a && . ../.env && set +a && node redeploy-fresh-osmosis.mjs
 */
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadMnemonic } from "./load-mnemonic.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const WASM_DIR = resolve(ROOT, "target/wasm32-unknown-unknown/release");
const PROD_ENV = resolve(
  ROOT,
  "../../www/apps/aquachain/src/environments/environment.prod.ts",
);

const uploadFee = {
  amount: [{ denom: DENOM, amount: "2500000" }],
  gas: "10000000",
};
const instantiateFee = {
  amount: [{ denom: DENOM, amount: "500000" }],
  gas: "2000000",
};

async function deploy(client, admin, label, wasmFile, initMsg) {
  const wasm = readFileSync(resolve(WASM_DIR, wasmFile));
  console.log(`\nDeploy ${label}…`);
  const upload = await client.upload(admin, wasm, uploadFee);
  const inst = await client.instantiate(
    admin,
    upload.codeId,
    initMsg,
    label,
    instantiateFee,
    { admin },
  );
  console.log("  ", inst.contractAddress);
  return inst.contractAddress;
}

const mnemonic = loadMnemonic();
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "osmo" });
const [acc] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const admin = acc.address;

const bal = await client.getBalance(admin, DENOM);
console.log("Admin", admin, (Number(bal.amount) / 1e6).toFixed(2), "OSMO");

const initDenom = { denom: DENOM };
const initDao = { quorum_bps: 3000, voting_period_seconds: 604800 };

const addresses = {
  CitizenScienceContractAddress: await deploy(
    client,
    admin,
    "citizen-science-registry-v2",
    "citizen_science_registry.wasm",
    initDenom,
  ),
  WaterWellContractAddress: await deploy(
    client,
    admin,
    "water-well-initiative-v2",
    "water_well_initiative.wasm",
    initDenom,
  ),
  UtilityWaterFootprintContractAddress: await deploy(
    client,
    admin,
    "utility-water-footprint-v2",
    "utility_water_footprint.wasm",
    initDenom,
  ),
  SustainableActionRewardsContractAddress: await deploy(
    client,
    admin,
    "sustainable-action-rewards-v2",
    "sustainable_action_rewards.wasm",
    initDenom,
  ),
  CommunityBountyContractAddress: await deploy(
    client,
    admin,
    "community-bounty-v2",
    "community_bounty.wasm",
    initDenom,
  ),
  WaterCreditMarketplaceContractAddress: await deploy(
    client,
    admin,
    "water-credit-marketplace-v2",
    "water_credit_marketplace.wasm",
    initDenom,
  ),
  LocalDaoContractAddress: await deploy(
    client,
    admin,
    "local-dao-v2",
    "local_dao.wasm",
    initDao,
  ),
  CrossPlatformExchangeContractAddress: await deploy(
    client,
    admin,
    "cross-platform-exchange-v2",
    "cross_platform_exchange.wasm",
    initDenom,
  ),
};

writeFileSync(
  resolve(dirname(fileURLToPath(import.meta.url)), "deployed-addresses.json"),
  JSON.stringify(addresses, null, 2),
);

let prod = readFileSync(PROD_ENV, "utf8");
for (const [key, addr] of Object.entries(addresses)) {
  prod = prod.replace(
    new RegExp(`${key}:\\s*\\n\\s*'[^']+'`),
    `${key}:\n    '${addr}'`,
  );
}
writeFileSync(PROD_ENV, prod);

const end = await client.getBalance(admin, DENOM);
console.log("\nAll deployed. Admin OSMO left:", (Number(end.amount) / 1e6).toFixed(2));
console.log(JSON.stringify(addresses, null, 2));
