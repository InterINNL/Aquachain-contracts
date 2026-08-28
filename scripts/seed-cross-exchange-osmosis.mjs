#!/usr/bin/env node
/**
 * Seed demo Cross Platform Exchange partners on Osmosis testnet.
 *
 *   set -a && . ../.env && set +a
 *   CONTRACT=osmo1… node seed-cross-exchange-osmosis.mjs
 */
import { readFileSync } from "node:fs";
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
const ONE_OSMO = "1000000";

const fee = {
  amount: [{ denom: DENOM, amount: "500000" }],
  gas: "1500000",
};

const partners = [
  {
    denom: "gujarat-water-unit",
    label: "Gujarat regional water ledger",
    region: "Ahmedabad, Gujarat, India",
    partner_amount: "100",
    rateLabel: "Gujarat: 1 OSMO = 100 ledger units",
  },
  {
    denom: "yamuna-credit",
    label: "Yamuna basin conservation credits",
    region: "Delhi NCR, India",
    partner_amount: "50",
    rateLabel: "Yamuna: 1 OSMO = 50 ledger units",
  },
  {
    denom: "bengaluru-aqua-point",
    label: "Bengaluru municipal aqua points",
    region: "Bengaluru, Karnataka, India",
    partner_amount: "75",
    rateLabel: "Bengaluru: 1 OSMO = 75 ledger units",
  },
  {
    denom: "udaipur-lake-point",
    label: "Udaipur lake stewardship points",
    region: "Udaipur, Rajasthan, India",
    partner_amount: "40",
    rateLabel: "Udaipur: 1 OSMO = 40 ledger units",
  },
];

function loadMnemonicFromEnvFile() {
  const envPath = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../.env",
  );
  try {
    const line = readFileSync(envPath, "utf8")
      .split(/\r?\n/)
      .find((row) => row.startsWith("MNEMONIC="));
    if (!line) {
      return "";
    }
    const raw = line.slice("MNEMONIC=".length).trim();
    if (
      (raw.startsWith("'") && raw.endsWith("'")) ||
      (raw.startsWith('"') && raw.endsWith('"'))
    ) {
      return raw.slice(1, -1).trim();
    }
    return raw;
  } catch {
    return "";
  }
}

async function loadWallet() {
  const mnemonic = process.env.MNEMONIC?.trim() || loadMnemonicFromEnvFile();
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
  console.error("Set MNEMONIC in contracts/.env or MNEMONIC / PRIVATE_KEY env");
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

async function exec(msg, label = "", funds = []) {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label, funds);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

async function partnerExists(denom) {
  try {
    await query({ get_partner: { partner_denom: denom } });
    return true;
  } catch {
    return false;
  }
}

async function rateMatches(denom, partnerAmount) {
  try {
    const rate = await query({ get_rate: { partner_denom: denom } });
    return rate.base_amount === ONE_OSMO && rate.partner_amount === partnerAmount;
  } catch {
    return false;
  }
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

for (const partner of partners) {
  if (!(await partnerExists(partner.denom))) {
    await exec(
      {
        register_partner: {
          denom: partner.denom,
          label: partner.label,
          region: partner.region,
        },
      },
      `${partner.denom} partner`,
    );
  } else {
    console.log("skip register", partner.denom, "(already exists)");
  }

  if (!(await rateMatches(partner.denom, partner.partner_amount))) {
    await exec(
      {
        set_rate: {
          partner_denom: partner.denom,
          base_amount: ONE_OSMO,
          partner_amount: partner.partner_amount,
        },
      },
      partner.rateLabel,
    );
  } else {
    console.log("skip rate", partner.denom, "(already 1 OSMO rate)");
  }
}

const listed = await query({ list_partners: { limit: 10 } });
console.log(
  "Partners",
  listed.map((p) => ({
    denom: p.denom,
    label: p.label,
    region: p.region,
  })),
);

// Demo liquidity: lock Gujarat units and fund contract pool for reverse swaps.
const demoOsmo = process.env.DEMO_SWAP_OSMO ?? "3";
const demoMicro = String(Number(demoOsmo) * 1_000_000);
try {
  const balance = await client.getBalance(me, DENOM);
  if (BigInt(balance.amount) > BigInt(demoMicro) + 1_000_000n) {
    await exec(
      {
        swap: {
          partner_denom: "gujarat-water-unit",
          direction: { base_to_partner: {} },
          amount: demoMicro,
        },
      },
      `Demo swap ${demoOsmo} OSMO → Gujarat units (liquidity)`,
      [coin(Number(demoMicro), DENOM)],
    );
  } else {
    console.log("skip demo swap (low balance)");
  }
} catch (err) {
  console.log("skip demo swap", err.message?.slice(0, 80));
}
