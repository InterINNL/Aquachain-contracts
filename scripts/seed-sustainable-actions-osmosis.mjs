#!/usr/bin/env node
/**
 * Seed demo Sustainable Action Rewards on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-sustainable-actions-osmosis.mjs
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
const { coin } = require("@cosmjs/amino");

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

async function exec(msg, funds = [], label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label, funds);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

function actionIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "action_id");
  return attr ? Number(attr.value) : null;
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

try {
  await exec({ add_verifier: { verifier: me } }, [], "add self as verifier");
} catch (e) {
  console.log("  (verifier)", e.message?.slice(0, 120) ?? e);
}

const demos = [
  {
    title: "Yamuna riverbank cleanup — Okhla",
    location: "Delhi, India",
    description: "Volunteer litter removal along the Yamuna floodplain.",
    impact_points: "120",
    verify: true,
    reward: "50000",
  },
  {
    title: "Lake Pichola restoration — Udaipur",
    location: "Rajasthan, India",
    description: "Floating debris collection and native planting.",
    impact_points: "80",
    verify: true,
    reward: null,
  },
  {
    title: "Mumbai beach plastic sweep — Juhu",
    location: "Maharashtra, India",
    description: "Weekend community cleanup with weigh-in evidence.",
    impact_points: "65",
    verify: false,
    reward: null,
  },
];

const ids = [];
for (const demo of demos) {
  const res = await exec(
    {
      submit_action: {
        evidence: {
          title: demo.title,
          location: demo.location,
          description: demo.description,
          impact_points: demo.impact_points,
        },
      },
    },
    [],
    demo.title,
  );
  const id = actionIdFrom(res);
  ids.push({ id, ...demo });
  console.log("  action_id", id);
}

for (const demo of ids) {
  if (!demo.verify || demo.id == null) continue;
  await exec({ verify_action: { action_id: demo.id } }, [], `verify ${demo.id}`);
  if (demo.reward) {
    await exec(
      { reward_actor: { action_id: demo.id } },
      [coin(Number(demo.reward), DENOM)],
      `reward ${demo.id}`,
    );
  }
}

const listed = await query({ list_actions: { start_after: null, limit: 20 } });
console.log(
  "Actions",
  listed.map((a) => ({
    id: a.id,
    verified: a.verified,
    rewarded: a.rewarded,
    title: (() => {
      try {
        return JSON.parse(a.evidence_str).title;
      } catch {
        return a.evidence_str?.slice(0, 40);
      }
    })(),
  })),
);
