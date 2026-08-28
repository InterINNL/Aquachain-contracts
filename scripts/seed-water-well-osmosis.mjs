#!/usr/bin/env node
/**
 * Seed demo Water Well projects on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-water-well-osmosis.mjs
 *   MNEMONIC='…' CONTRACT=osmo1… node seed-water-well-osmosis.mjs
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
const { coins } = require("@cosmjs/amino");

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const CONTRACT =
  process.env.CONTRACT ??
  "osmo1d3ulz45nqswlvrf7l4cj7ul5ky0pw7s3cqcdxnj63gpwkjuaszzsa2w9ta";

const fee = {
  amount: [{ denom: DENOM, amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};


const wallet = await loadWallet();
const [account] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const me = account.address;

async function exec(msg, funds = [], label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label, funds);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

function projectIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "project_id");
  return attr ? Number(attr.value) : null;
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

const demos = [
  {
    label: "proposed — leave pending",
    goal: "2500000",
    data: {
      title: "Village handpump — Udaipur district",
      location: "Rajasthan, India",
      description:
        "Replace a failed handpump serving a rural hamlet outside Udaipur.",
    },
    after: "proposed",
  },
  {
    label: "fundraising — partial donate",
    goal: "1000000",
    data: {
      title: "School borehole — Bengaluru outskirts",
      location: "Karnataka, India",
      description:
        "Drill and equip a solar borehole for a peri-urban school block.",
    },
    after: "partial",
    donate: "350000",
  },
  {
    label: "disbursable — fully funded + unlock",
    goal: "500000",
    data: {
      title: "Solar pump — Nashik farm cooperative",
      location: "Maharashtra, India",
      description: "Install a solar pump and storage tank for a farm cooperative.",
    },
    after: "disbursable",
  },
  {
    label: "completed — fund unlock disburse",
    goal: "200000",
    data: {
      title: "Community well rehab — Gujarat",
      location: "Gujarat, India",
      description: "Rehabilitate an existing well and add a child-safe apron.",
    },
    after: "completed",
  },
  {
    label: "cancelled — admin cancel",
    goal: "750000",
    data: {
      title: "Salt lake survey — Sundarbans fringe",
      location: "West Bengal, India",
      description: "Withdrawn after mangrove protection review in Kolkata region.",
    },
    after: "cancelled",
  },
];

const ids = [];

for (const demo of demos) {
  const res = await exec(
    { create_project: { goal: demo.goal, data: demo.data } },
    [],
    `create ${demo.data.title}`,
  );
  const id = projectIdFrom(res);
  if (id == null) throw new Error("missing project_id");
  ids.push({ id, ...demo });
  console.log("  project_id", id);
}

for (const demo of ids) {
  if (demo.after === "proposed") continue;

  if (demo.after === "cancelled") {
    await exec({ cancel: { project_id: demo.id } }, [], `cancel ${demo.id}`);
    continue;
  }

  await exec({ validate: { project_id: demo.id } }, [], `validate ${demo.id}`);

  if (demo.after === "partial") {
    await exec(
      { donate: { project_id: demo.id } },
      coins(Number(demo.donate), DENOM),
      `donate partial ${demo.id}`,
    );
    continue;
  }

  await exec(
    { donate: { project_id: demo.id } },
    coins(Number(demo.goal), DENOM),
    `donate full ${demo.id}`,
  );
  await exec({ unlock: { project_id: demo.id } }, [], `unlock ${demo.id}`);

  if (demo.after === "completed") {
    await exec({ disburse: { project_id: demo.id } }, [], `disburse ${demo.id}`);
  }
}

const listed = await query({ list_projects: { start_after: null, limit: 20 } });
const counts = await query({ get_project_status_counts: {} });
console.log("Status counts", counts);
console.log(
  "Projects",
  listed.map((p) => ({
    id: p.id,
    status: p.status,
    goal: p.goal,
    donated: p.total_donated,
    title: (() => {
      try {
        return JSON.parse(p.data_str).title;
      } catch {
        return p.data_str?.slice(0, 40);
      }
    })(),
  })),
);
