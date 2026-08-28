#!/usr/bin/env node
/**
 * Seed demo field agents and drone readings on citizen-science-registry (G3).
 *
 *   set -a && . ../.env && set +a
 *   CONTRACT=osmo1… node seed-agents-osmosis.mjs
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
const {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} = require("@cosmjs/proto-signing");
const { stringToPath } = require("@cosmjs/crypto");

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

const mnemonic = loadMnemonic();
let verifierOperator = me;
const verifierWallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
  prefix: "osmo",
  hdPaths: [stringToPath("m/44'/118'/1'/0/0")],
});
const [verifierAccount] = await verifierWallet.getAccounts();
verifierOperator = verifierAccount.address;

async function exec(msg, funds = [], label = "") {
  console.log("→", label || JSON.stringify(msg).slice(0, 100));
  const res = await client.execute(me, CONTRACT, msg, fee, label, funds);
  console.log("  tx", res.transactionHash);
  return res;
}

async function query(msg) {
  return client.queryContractSmart(CONTRACT, msg);
}

function sensorIdFrom(res) {
  const attr = res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === "sensor_id");
  return attr ? Number(attr.value) : null;
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

const agents = [
  {
    name: "YamunaDrone-1",
    agent_type: "drone",
    operator: me,
    policy: {
      site: "Yamuna Wazirabad barrage, Delhi NCR, India",
      max_turbidity_ntu: "25",
      flight_profile: "river_transect",
    },
  },
  {
    name: "VerifyBot-A",
    agent_type: "verifier",
    operator: verifierOperator,
    policy: {
      site: "India demo verifier",
      auto_approve_below_ntu: "18",
    },
  },
];

for (const agent of agents) {
  try {
    await exec(
      {
        register_agent: {
          name: agent.name,
          agent_type: agent.agent_type,
          operator: agent.operator,
          pubkey: `demo-${agent.name.toLowerCase()}-v1`,
          policy: agent.policy,
        },
      },
      [],
      `register ${agent.name}`,
    );
  } catch (e) {
    console.log("  (agent)", e.message?.slice(0, 120) ?? e);
  }
}

try {
  await exec({ add_verifier: { verifier: verifierOperator } }, [], "add verifier");
} catch (e) {
  console.log("  (verifier)", e.message?.slice(0, 120) ?? e);
}

const droneSensorPayload = {
  type: "Drone River Monitor",
  model: "YamunaDrone-1",
  location: {
    lat: "28.7041",
    lon: "77.1025",
    name: "Yamuna Wazirabad barrage, Delhi, India",
  },
  parameters: ["turbidity", "temperature", "gps"],
  agent: "YamunaDrone-1",
};

let sensorId = null;
try {
  const res = await exec(
    { submit_sensor: { data: droneSensorPayload } },
    [],
    "register Yamuna drone sensor",
  );
  sensorId = sensorIdFrom(res);
} catch (e) {
  console.log("  (sensor)", e.message?.slice(0, 120) ?? e);
}

if (sensorId) {
  try {
    await exec({ activate: { sensor_id: sensorId } }, [], "activate drone sensor");
  } catch (e) {
    console.log("  (activate)", e.message?.slice(0, 120) ?? e);
  }

  const readings = [
    {
      lat: "28.7041",
      lon: "77.1025",
      turbidity: "14.2",
      image_hash: "sha256:demo-yamuna-frame-001",
      flight_id: "yamuna-drone-001",
      unit: "NTU",
      site: "Yamuna Wazirabad barrage, Delhi NCR, India",
    },
    {
      lat: "12.9716",
      lon: "77.5946",
      turbidity: "11.8",
      image_hash: "sha256:demo-bellandur-frame-002",
      flight_id: "bengaluru-drone-002",
      unit: "NTU",
      site: "Bellandur lake edge, Bengaluru, India",
    },
    {
      lat: "19.0760",
      lon: "72.8777",
      turbidity: "16.5",
      image_hash: "sha256:demo-mithi-frame-003",
      flight_id: "mithi-drone-003",
      unit: "NTU",
      site: "Mithi river estuary, Mumbai, India",
    },
  ];

  for (const reading of readings) {
    try {
      await exec(
        { submit_data: { sensor_id: sensorId, data: reading } },
        [],
        `reading ${reading.flight_id}`,
      );
    } catch (e) {
      console.log("  (reading)", e.message?.slice(0, 120) ?? e);
    }
  }
}

const listedAgents = await query({ list_agents: { limit: 10 } });
console.log(
  "Agents",
  listedAgents.map((a) => ({
    id: a.id,
    name: a.name,
    agent_type: a.agent_type,
    operator: a.operator,
  })),
);
