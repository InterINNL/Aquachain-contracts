#!/usr/bin/env node
/**
 * Run per-module seed scripts against fa75-admin contracts (post-redeploy).
 * Adds 2s pause between scripts to avoid RPC 403 bursts.
 *
 *   set -a && . ../.env && set +a && node seed-modules-osmosis.mjs
 */
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function loadAddresses() {
  const path = resolve(dir, "deployed-addresses.json");
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return {};
  }
}

const deployed = loadAddresses();

const jobs = [
  ["seed-osmosis.mjs", "CS", deployed.CitizenScienceContractAddress],
  ["seed-water-well-osmosis.mjs", "WW", deployed.WaterWellContractAddress],
  ["seed-footprint-osmosis.mjs", "WU", deployed.UtilityWaterFootprintContractAddress],
  ["seed-sustainable-actions-osmosis.mjs", "SA", deployed.SustainableActionRewardsContractAddress],
  ["seed-community-bounty-osmosis.mjs", "CB", deployed.CommunityBountyContractAddress],
  ["seed-water-credits-osmosis.mjs", "WC", deployed.WaterCreditMarketplaceContractAddress],
  ["seed-local-dao-osmosis.mjs", "DAO", deployed.LocalDaoContractAddress],
  ["seed-cross-exchange-osmosis.mjs", "XC", deployed.CrossPlatformExchangeContractAddress],
].filter(([, , addr]) => Boolean(addr));

for (const [script, label, contract] of jobs) {
  console.log(`\n======== ${label} ${contract.slice(0, 18)}… ========`);
  try {
    execSync(`node ${script}`, {
      cwd: dir,
      stdio: "inherit",
      env: {
        ...process.env,
        RPC: process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone",
        FEE_AMOUNT: process.env.FEE_AMOUNT ?? "80000",
        CONTRACT: contract,
        DEMO_SWAP_OSMO: label === "XC" ? "1" : process.env.DEMO_SWAP_OSMO ?? "",
      },
    });
  } catch (e) {
    console.error(`${label} failed (continuing)`, e.message?.slice(0, 80));
  }
  await sleep(3000);
}

console.log("\nDone seed-modules-osmosis");
