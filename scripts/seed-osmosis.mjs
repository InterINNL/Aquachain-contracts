#!/usr/bin/env node
/**
 * Seed demo activity on Osmosis citizen-science-registry.
 *
 *   MNEMONIC='…' CONTRACT=osmo1… node seed-osmosis.mjs
 *   PRIVATE_KEY='…hex…' CONTRACT=osmo1… node seed-osmosis.mjs
 */
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");
const { coins } = require("@cosmjs/amino");

const RPC = process.env.RPC ?? "https://rpc.testnet.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const CONTRACT =
  process.env.CONTRACT ??
  "osmo1nqqev3y5l8sjgrghuplagy0td3tdcy0cklx9mqnze27j2ynu7jrqram74j";

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

const wallet = await loadWallet();
const [account] = await wallet.getAccounts();
const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet);
const me = account.address;

async function exec(msg, funds = [], label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 80));
  const res = await client.execute(me, CONTRACT, msg, fee, label, funds);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

// 1) Self as verifier (admin = instantiator)
try {
  await exec({ add_verifier: { verifier: me } }, [], "add verifier");
} catch (e) {
  console.log("  (verifier)", e.message?.slice(0, 120) ?? e);
}

// 2) Register a few sensors (unique payloads)
// CosmWasm JSON rejects bare floats; keep numbers as strings (matches Makefile demos).
const sensors = [
  {
    type: "Water Quality",
    model: "demo-seine-up",
    location: { lat: "48.86", lng: "2.35", description: "Seine Upstream" },
  },
  {
    type: "Water Quality",
    model: "demo-seine-down",
    location: { lat: "48.84", lng: "2.37", description: "Seine Downstream" },
  },
  {
    type: "Water Level",
    model: "demo-well-alpha",
    location: { lat: "48.87", lng: "2.33", description: "Well Alpha" },
  },
];

const sensorIds = [];
for (const data of sensors) {
  const label = data.location?.description ?? data.model;
  const res = await exec({ submit_sensor: { data } }, [], `sensor ${label}`);
  const idAttr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "sensor_id");
  const id = idAttr ? Number(idAttr.value) : sensorIds.length + 1;
  sensorIds.push(id);
  console.log("  sensor_id", id);
}

// 3) Activate (admin)
for (const id of sensorIds) {
  await exec({ activate: { sensor_id: id } }, [], `activate ${id}`);
}

// 4) Submit readings
const entries = [];
for (const id of sensorIds) {
  for (const value of ["21.5", "22.1", "23.0"]) {
    const res = await exec(
      { submit_data: { sensor_id: id, data: { value, unit: "C" } } },
      [],
      `data sensor ${id} = ${value}`,
    );
    const entryAttr = res.events
      .flatMap((e) => e.attributes)
      .find((a) => a.key === "entry_id");
    if (entryAttr) entries.push(Number(entryAttr.value));
  }
}

// 5) Verify first few entries
for (const entryId of entries.slice(0, 4)) {
  try {
    await exec({ verify_data: { entry_id: entryId } }, [], `verify ${entryId}`);
  } catch (e) {
    console.log("  (verify)", e.message?.slice(0, 120) ?? e);
  }
}

// 6) Reward one verified entry (send uosmo with msg)
if (entries[0]) {
  try {
    await exec(
      { reward_submitter: { entry_id: entries[0] } },
      coins(10_000, DENOM),
      `reward ${entries[0]}`,
    );
  } catch (e) {
    console.log("  (reward)", e.message?.slice(0, 120) ?? e);
  }
}

const listedSensors = await query({
  list_sensors: { start_after: null, limit: 30 },
});
const listedData = await query({
  list_data_entries: { start_after: null, limit: 30 },
});
console.log("Sensors count", listedSensors.length);
console.log(
  "Sensors",
  listedSensors.map((s) => ({
    id: s.id,
    status: s.status,
    data: s.data_str?.slice(0, 60),
  })),
);
console.log("Data entries", listedData.length);
console.log(
  "Entries",
  listedData.map((e) => ({
    id: e.id,
    sensor_id: e.sensor_id,
    verified: e.verified,
    rewarded: e.rewarded,
  })),
);
