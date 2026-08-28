#!/usr/bin/env node
/**
 * Seed demo Cross Platform Exchange partners on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-cross-exchange-osmosis.mjs
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

const RPC = process.env.RPC ?? "https://rpc.testnet.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const CONTRACT = process.env.CONTRACT ?? "";

const fee = {
  amount: [{ denom: DENOM, amount: "50000" }],
  gas: "500000",
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

async function exec(msg, label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

await exec(
  {
    register_partner: {
      denom: "gujarat-water-unit",
      label: "Gujarat regional water ledger",
      region: "Ahmedabad, Gujarat, India",
    },
  },
  "Gujarat partner",
);

await exec(
  {
    set_rate: {
      partner_denom: "gujarat-water-unit",
      base_amount: "1000000",
      partner_amount: "100",
    },
  },
  "Gujarat rate (1 OSMO = 100 units)",
);

await exec(
  {
    register_partner: {
      denom: "yamuna-credit",
      label: "Yamuna basin conservation credits",
      region: "Delhi NCR, India",
    },
  },
  "Yamuna partner",
);

await exec(
  {
    set_rate: {
      partner_denom: "yamuna-credit",
      base_amount: "500000",
      partner_amount: "25",
    },
  },
  "Yamuna rate",
);

await exec(
  {
    register_partner: {
      denom: "bengaluru-aqua-point",
      label: "Bengaluru municipal aqua points",
      region: "Bengaluru, Karnataka, India",
    },
  },
  "Bengaluru partner",
);

await exec(
  {
    set_rate: {
      partner_denom: "bengaluru-aqua-point",
      base_amount: "2000000",
      partner_amount: "50",
    },
  },
  "Bengaluru rate",
);

await exec(
  {
    register_partner: {
      denom: "udaipur-lake-point",
      label: "Udaipur lake stewardship points",
      region: "Udaipur, Rajasthan, India",
    },
  },
  "Udaipur partner",
);

await exec(
  {
    set_rate: {
      partner_denom: "udaipur-lake-point",
      base_amount: "1000000",
      partner_amount: "40",
    },
  },
  "Udaipur rate",
);

const partners = await query({ list_partners: { limit: 10 } });
console.log(
  "Partners",
  partners.map((p) => ({
    denom: p.denom,
    label: p.label,
    region: p.region,
  })),
);
