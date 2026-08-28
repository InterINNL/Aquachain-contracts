#!/usr/bin/env node
/**
 * Demo Local DAO v2 post_bounty flow on Osmosis testnet.
 *
 * Wires action targets (admin), creates a post_bounty proposal, and votes yes.
 * execute_proposal requires voting_end (default 7 days on prod deploy).
 *
 *   set -a && . ../.env && set +a
 *   node demo-dao-post-bounty-osmosis.mjs
 *
 * Optional overrides:
 *   LOCAL_DAO=osmo1… COMMUNITY_BOUNTY=osmo1… CSR=osmo1… WCM=osmo1…
 */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadWallet } from "./load-wallet.mjs";
import { loadMnemonic } from "./load-mnemonic.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");
const { stringToPath } = require("@cosmjs/crypto");

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const LOCAL_DAO =
  process.env.LOCAL_DAO ??
  "osmo1wyf5hpwpkuml6dtzj7ldek220pqy73j8a4crkp9pdupttrzfs2rq5dtwms";
const COMMUNITY_BOUNTY =
  process.env.COMMUNITY_BOUNTY ??
  "osmo1yjgevdft34e36g0ltf64jzy3e3p59c2tmtz4nm06wnh25448t8espvxmxx";
const CSR =
  process.env.CSR ??
  "osmo1s2a2f6je78ga2atc3rzq76lmzs9kane9h9fa3w62rjvcw5mk5lvsyxwtm4";
const WCM =
  process.env.WCM ??
  "osmo1cfymf04ufxh8c5z229v39skdyevy3tw87ywjukfxq6hr7wn7zfwq74c0p6";

const fee = {
  amount: [{ denom: DENOM, amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};

const wallet = await loadWallet();
const [account] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const me = account.address;

const mnemonic = loadMnemonic();
const voterWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
  prefix: "osmo",
  hdPaths: [stringToPath("m/44'/118'/1'/0/0")],
});
const [voterAccount] = await voterWallet.getAccounts();
const voterClient = await SigningCosmWasmClient.connectWithSigner(
  RPC,
  voterWallet,
);
const voter = voterAccount.address;

async function exec(signerClient, signer, msg, label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await signerClient.execute(signer, LOCAL_DAO, msg, fee, label);
  console.log("  tx", res.transactionHash);
  return res;
}

function proposalIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "proposal_id");
  return attr ? Number(attr.value) : null;
}

console.log("Admin   ", me);
console.log("Voter   ", voter);
console.log("LocalDAO", LOCAL_DAO);

try {
  await exec(
    client,
    me,
    {
      update_action_targets: {
        community_bounty: COMMUNITY_BOUNTY,
        water_credit_marketplace: WCM,
        citizen_science_registry: CSR,
      },
    },
    "wire action targets",
  );
} catch (e) {
  console.log("  (wire)", e.message?.slice(0, 120) ?? e);
}

const deadline = Math.floor(Date.now() / 1000) + 86400 * 30;
const created = await exec(
  client,
  me,
  {
    create_proposal: {
      title: "Yamuna cleanup bounty — Phase G smoke",
      description: "Fund riverbank litter removal after drone turbidity alert.",
      action_tag: "post_bounty",
      metadata: {
        location: "Yamuna Wazirabad, Delhi, India",
        deadline,
        reward: "5000000",
      },
    },
  },
  "create post_bounty proposal",
);
const proposalId = proposalIdFrom(created);
if (!proposalId) {
  console.error("No proposal_id in tx events");
  process.exit(1);
}

for (const [signerClient, signer, who] of [
  [client, me, "admin"],
  [voterClient, voter, "voter"],
]) {
  try {
    await exec(
      signerClient,
      signer,
      { vote: { proposal_id: proposalId, vote: "yes" } },
      `vote yes (${who})`,
    );
  } catch (e) {
    console.log(`  (vote ${who})`, e.message?.slice(0, 120) ?? e);
  }
}

const listed = await client.queryContractSmart(LOCAL_DAO, {
  list_proposals: { limit: 5 },
});
console.log(
  "Proposals",
  listed.map((p) => ({
    id: p.id,
    title: p.title,
    status: p.status,
    action_tag: p.action_tag,
    yes_votes: p.yes_votes,
    voting_end: p.voting_end,
  })),
);
console.log(
  "\nNext: after voting_end, execute with 5 OSMO attached:",
  `execute_proposal { proposal_id: ${proposalId} } + funds 5000000 uosmo`,
);
