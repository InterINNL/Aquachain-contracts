#!/usr/bin/env node
/**
 * Seed demo Local DAO proposals on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-local-dao-osmosis.mjs
 */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadWallet } from "./load-wallet.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const CONTRACT = process.env.CONTRACT ?? "";

const fee = {
  amount: [{ denom: DENOM, amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};


if (!CONTRACT) {
  console.error("Set CONTRACT=osmo1…");
  process.exit(1);
}

const wallet = await loadWallet();
const [account] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const me = account.address;

async function exec(msg, label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

function proposalIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "proposal_id");
  return attr ? Number(attr.value) : null;
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

const p1 = await exec(
  {
    create_proposal: {
      title: "Rainwater harvesting mandate — Dwarka sector",
      description:
        "Require new residential blocks to install rooftop rainwater tanks with shared overflow monitoring.",
      action_tag: "policy_rainwater",
      metadata: {
        location: "Dwarka, Delhi, India",
        summary: "Municipal pilot for 12 housing societies",
      },
    },
  },
  "Dwarka rainwater proposal",
);
console.log("  proposal_id", proposalIdFrom(p1));

const p2 = await exec(
  {
    create_proposal: {
      title: "Bellandur lake buffer restoration fund",
      description:
        "Allocate treasury OSMO for native plant buffers and weekly litter audits along the lake edge.",
      action_tag: "fund_restoration",
      metadata: {
        location: "Bellandur, Bengaluru, India",
        summary: "Quarterly community audit schedule",
      },
    },
  },
  "Bellandur restoration proposal",
);
console.log("  proposal_id", proposalIdFrom(p2));

const p3 = await exec(
  {
    create_proposal: {
      title: "Smart meter rollout — Fatehpura ward",
      description:
        "Install IoT flow meters on community standposts and publish readings to the citizen science registry.",
      action_tag: "deploy_meters",
      metadata: {
        location: "Fatehpura, Udaipur, Rajasthan, India",
        summary: "Pilot 24 standposts with open dashboards",
      },
    },
  },
  "Fatehpura meter proposal",
);
console.log("  proposal_id", proposalIdFrom(p3));

const p4 = await exec(
  {
    create_proposal: {
      title: "Mithi river plastic barrier maintenance",
      description:
        "Fund monthly servicing of floating trash barriers and publish weight logs for recyclers.",
      action_tag: "maintain_barrier",
      metadata: {
        location: "Mithi river, Mumbai, India",
        summary: "Partner with local recycler co-op",
      },
    },
  },
  "Mithi barrier proposal",
);
console.log("  proposal_id", proposalIdFrom(p4));

const listed = await query({ list_proposals: { limit: 10 } });
console.log(
  "Proposals",
  listed.map((p) => ({
    id: p.id,
    title: p.title,
    status: p.status,
    location: JSON.parse(p.metadata_str).location,
  })),
);
