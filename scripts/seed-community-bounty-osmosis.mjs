#!/usr/bin/env node
/**
 * Seed demo Community Bounty tasks on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-community-bounty-osmosis.mjs
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

function bountyIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "bounty_id");
  return attr ? Number(attr.value) : null;
}

function submissionIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "submission_id");
  return attr ? Number(attr.value) : null;
}

const now = Math.floor(Date.now() / 1000);
const twoWeeks = now + 14 * 86_400;
const oneWeek = now + 7 * 86_400;

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

const b1 = await exec(
  {
    post_bounty: {
      title: "Yamuna riverbank cleanup — Okhla",
      description:
        "Remove plastic and debris from a 500 m stretch. Photo evidence and weigh-in required.",
      location: "Okhla, Delhi, India",
      deadline: twoWeeks,
    },
  },
  [coin(500_000, DENOM)],
  "Yamuna cleanup bounty",
);
const id1 = bountyIdFrom(b1);
console.log("  bounty_id", id1);

const b2 = await exec(
  {
    post_bounty: {
      title: "Lake Pichola litter pick — Udaipur",
      description:
        "Sort and bag litter along the lake walking path. NGO coordinator sign-off required.",
      location: "Udaipur, Rajasthan, India",
      deadline: twoWeeks,
    },
  },
  [coin(350_000, DENOM)],
  "Lake Pichola bounty",
);
const id2 = bountyIdFrom(b2);
console.log("  bounty_id", id2);

const b3 = await exec(
  {
    post_bounty: {
      title: "Juhu beach plastic sweep — Mumbai",
      description:
        "Morning shift with local volunteers. Log kg collected and attach photo set.",
      location: "Juhu, Mumbai, India",
      deadline: oneWeek,
    },
  },
  [coin(750_000, DENOM)],
  "Juhu beach bounty",
);
const id3 = bountyIdFrom(b3);
console.log("  bounty_id", id3);

const s1 = await exec(
  {
    submit_work: {
      bounty_id: id1,
      work: {
        summary: "Completed 500 m sweep with 38 kg plastic logged",
        location: "Okhla, Delhi, India",
        evidence: "Photo album + NGO weigh-in sheet hash",
        hours_spent: "6",
      },
    },
  },
  [],
  "submit work for bounty 1",
);
const sub1 = submissionIdFrom(s1);
console.log("  submission_id", sub1);

await exec(
  { approve_work: { bounty_id: id1, submission_id: sub1 } },
  [],
  "approve submission 1",
);

const listed = await query({ list_bounties: { limit: 10 } });
console.log(
  "Bounties",
  listed.map((b) => ({
    id: b.id,
    title: b.title,
    status: b.status,
    reward: b.reward_amount,
  })),
);
