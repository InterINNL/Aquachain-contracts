#!/usr/bin/env node
/**
 * Seed demo Water Credit Marketplace on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-water-credits-osmosis.mjs
 */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");
const { coin } = require("@cosmjs/amino");

const RPC = process.env.RPC ?? "https://rpc.testnet.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const CONTRACT = process.env.CONTRACT ?? "";

const fee = {
  amount: [{ denom: DENOM, amount: "500000" }],
  gas: "1500000",
};

async function loadWallet() {
  const mnemonic = process.env.MNEMONIC?.trim();
  if (mnemonic) {
    return DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: "osmo" });
  }
  const privateKey = process.env.PRIVATE_KEY?.trim().replace(/^0x/i, "");
  if (privateKey && /^[0-9a-fA-F]{64}$/.test(privateKey)) {
    return DirectSecp256k1Wallet.fromKey(
      Uint8Array.from(Buffer.from(privateKey, "hex")),
      "osmo",
    );
  }
  console.error("Set MNEMONIC or PRIVATE_KEY");
  process.exit(1);
}

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

function listingIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "listing_id");
  return attr ? Number(attr.value) : null;
}

const now = Math.floor(Date.now() / 1000);
const twoWeeks = now + 14 * 86_400;

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

await exec(
  { mint_credits: { recipient: me, amount: "500" } },
  [],
  "mint credits to deployer",
);

const l1 = await exec(
  {
    list_credit: {
      credits: "120",
      price: "400000",
      region: "Delhi Jal Board — Yamuna basin, India",
      expires_at: twoWeeks,
    },
  },
  [],
  "list Delhi credits",
);
console.log("  listing_id", listingIdFrom(l1));

const l2 = await exec(
  {
    list_credit: {
      credits: "80",
      price: "250000",
      region: "BWSSB Bengaluru, Karnataka, India",
      expires_at: twoWeeks,
    },
  },
  [],
  "list Bengaluru credits",
);
console.log("  listing_id", listingIdFrom(l2));

const l3 = await exec(
  {
    list_credit: {
      credits: "200",
      price: "600000",
      region: "Udaipur lake conservation district, India",
      expires_at: twoWeeks,
    },
  },
  [],
  "list Udaipur credits",
);
const id3 = listingIdFrom(l3);
console.log("  listing_id", id3);

await exec(
  { buy_credit: { listing_id: id3 } },
  [coin(600_000, DENOM)],
  "buy Udaipur listing (self-demo)",
);

const balance = await query({ get_balance: { address: me } });
const listings = await query({ list_listings: { limit: 10 } });
console.log("Balance", balance);
console.log(
  "Listings",
  listings.map((l) => ({
    id: l.id,
    region: l.region,
    credits: l.credits,
    price: l.price,
    active: l.active,
  })),
);
