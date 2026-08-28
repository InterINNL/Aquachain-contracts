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
  "osmo1nqqev3y5l8sjgrghuplagy0td3tdcy0cklx9mqnze27j2ynu7jrqram74j";

const fee = {
  amount: [{ denom: DENOM, amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};


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
    model: "yamuna-wazirabad",
    location: {
      lat: "28.70",
      lng: "77.22",
      description: "Yamuna Wazirabad barrage, Delhi NCR, India",
    },
  },
  {
    type: "Water Quality",
    model: "sabarmati-ahmedabad",
    location: {
      lat: "23.02",
      lng: "72.57",
      description: "Sabarmati riverfront, Ahmedabad, Gujarat, India",
    },
  },
  {
    type: "Water Level",
    model: "lake-pichola-udaipur",
    location: {
      lat: "24.57",
      lng: "73.68",
      description: "Lake Pichola level gauge, Udaipur, Rajasthan, India",
    },
  },
  {
    type: "Water pH",
    model: "adyar-chennai",
    location: {
      lat: "13.00",
      lng: "80.25",
      description: "Adyar estuary, Chennai, Tamil Nadu, India",
    },
  },
  {
    type: "Water Turbidity",
    model: "hooghly-kolkata",
    location: {
      lat: "22.57",
      lng: "88.36",
      description: "Hooghly river monitoring, Kolkata, West Bengal, India",
    },
  },
  {
    type: "Water Temperature",
    model: "periyar-kochi",
    location: {
      lat: "9.97",
      lng: "76.28",
      description: "Periyar backwater inlet, Kochi, Kerala, India",
    },
  },
  {
    type: "Water Quantity",
    model: "krishna-vijayawada",
    location: {
      lat: "16.52",
      lng: "80.63",
      description: "Krishna river flow, Vijayawada, Andhra Pradesh, India",
    },
  },
  {
    type: "Water Quality",
    model: "mula-mutha-pune",
    location: {
      lat: "18.52",
      lng: "73.86",
      description: "Mula-Mutha confluence, Pune, Maharashtra, India",
    },
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
