#!/usr/bin/env node
/** Wait for voting_end then execute post_bounty proposal #1. */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { loadWallet } from "./load-wallet.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const {
  SigningCosmWasmClient,
  CosmWasmClient,
} = require("@cosmjs/cosmwasm-stargate");

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const LOCAL_DAO =
  process.env.LOCAL_DAO ??
  "osmo1uc2fd0vpkjfzcuu2xtxclj8ftaj2xezhlhxq7exmahtmlqyljxnsnnw7t3";
const BOUNTY =
  process.env.BOUNTY ??
  "osmo1yjgevdft34e36g0ltf64jzy3e3p59c2tmtz4nm06wnh25448t8espvxmxx";
const PROPOSAL_ID = Number(process.env.PROPOSAL_ID ?? "1");
const REWARD = process.env.REWARD ?? "5000000";

const fee = {
  amount: [{ denom: "uosmo", amount: "80000" }],
  gas: "1500000",
};

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

const query = await CosmWasmClient.connect(RPC);
const proposal = await query.queryContractSmart(LOCAL_DAO, {
  get_proposal: { proposal_id: PROPOSAL_ID },
});
const end = Number(proposal.voting_end);
const now = Math.floor(Date.now() / 1000);
console.log(
  "proposal",
  PROPOSAL_ID,
  "status",
  proposal.status,
  "yes",
  proposal.yes_votes,
);
console.log("voting_end", end, "now", now);

if (proposal.status === "executed") {
  console.log("Already executed.");
  process.exit(0);
}

while (Math.floor(Date.now() / 1000) <= end) {
  const left = end - Math.floor(Date.now() / 1000);
  console.log(`waiting ${left}s until voting ends…`);
  await sleep(Math.min(left + 2, 60) * 1000);
}

const wallet = await loadWallet();
const [acc] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const before = await query.queryContractSmart(BOUNTY, {
  list_bounties: { limit: 20 },
});

console.log("executing with", REWARD, "uosmo…");
const res = await client.execute(
  acc.address,
  LOCAL_DAO,
  { execute_proposal: { proposal_id: PROPOSAL_ID } },
  fee,
  "execute post_bounty",
  [{ denom: "uosmo", amount: REWARD }],
);
console.log("execute tx", res.transactionHash);

const after = await query.queryContractSmart(BOUNTY, {
  list_bounties: { limit: 20 },
});
const updated = await query.queryContractSmart(LOCAL_DAO, {
  get_proposal: { proposal_id: PROPOSAL_ID },
});
console.log(
  JSON.stringify(
    {
      proposal_status: updated.status,
      bounties_before: before.length,
      bounties_after: after.length,
      latest_title: after.at(-1)?.title,
    },
    null,
    2,
  ),
);
