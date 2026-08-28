#!/usr/bin/env node
/**
 * Redeploy fa75-admin contracts (where needed), seed all modules with Indian demo
 * data, fund demo actors, and print environment.prod.ts address block.
 *
 *   set -a && . ../.env && set +a && node seed-all-osmosis.mjs
 */
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { execSync } from "node:child_process";
import { loadMnemonic } from "./load-mnemonic.mjs";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "../../../www/package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");
const { stringToPath } = require("@cosmjs/crypto");
const { coin, coins } = require("@cosmjs/amino");

const RPC = process.env.RPC ?? "https://rpc.testnet.osmosis.zone";
const DENOM = process.env.DENOM ?? "uosmo";
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const WASM = resolve(ROOT, "target/wasm32-unknown-unknown/release");
const PROD_ENV = resolve(
  ROOT,
  "../../../www/apps/aquachain/src/environments/environment.prod.ts",
);

const fee = {
  amount: [{ denom: DENOM, amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};

const uploadFee = {
  amount: [{ denom: DENOM, amount: "2500000" }],
  gas: "10000000",
};

const instantiateFee = {
  amount: [{ denom: DENOM, amount: "500000" }],
  gas: "2000000",
};

async function walletAt(mnemonic, accountIndex = 0) {
  const path =
    accountIndex === 0
      ? stringToPath("m/44'/118'/0'/0/0")
      : stringToPath(`m/44'/118'/${accountIndex}'/0/0`);
  return DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "osmo",
    hdPaths: [path],
  });
}

async function deploy(client, admin, label, wasmFile, initMsg = { denom: DENOM }) {
  const wasm = readFileSync(resolve(WASM, wasmFile));
  console.log(`\n=== Deploy ${label} ===`);
  const upload = await client.upload(admin, wasm, uploadFee);
  const inst = await client.instantiate(
    admin,
    upload.codeId,
    initMsg,
    label,
    instantiateFee,
    { admin },
  );
  console.log("contract", inst.contractAddress);
  return inst.contractAddress;
}

async function exec(client, sender, contract, msg, label, funds = []) {
  console.log("→", label);
  try {
    const res = await client.execute(sender, contract, msg, fee, label, funds);
    console.log("  tx", res.transactionHash.slice(0, 16) + "…");
    return res;
  } catch (e) {
    console.log("  FAIL", e.message?.slice(0, 120) ?? e);
    return null;
  }
}

async function query(client, contract, msg) {
  return client.queryContractSmart(contract, msg);
}

function attr(res, key) {
  if (!res?.events) return undefined;
  return res.events
    .flatMap((e) => e.attributes)
    .find((a) => a.key === key)?.value;
}

const mnemonic = loadMnemonic();
const adminWallet = await walletAt(mnemonic, 0);
const volunteerWallet = await walletAt(mnemonic, 1);
const buyerWallet = await walletAt(mnemonic, 2);

const [adminAcc] = await adminWallet.getAccounts();
const [volunteerAcc] = await volunteerWallet.getAccounts();
const [buyerAcc] = await buyerWallet.getAccounts();

const admin = adminAcc.address;
const volunteer = volunteerAcc.address;
const buyer = buyerAcc.address;

console.log("Admin     ", admin);
console.log("Volunteer ", volunteer);
console.log("Buyer     ", buyer);

const adminClient = await SigningCosmWasmClient.connectWithSigner(RPC, adminWallet);
const volunteerClient = await SigningCosmWasmClient.connectWithSigner(
  RPC,
  volunteerWallet,
);
const buyerClient = await SigningCosmWasmClient.connectWithSigner(RPC, buyerWallet);

const balStart = await adminClient.getBalance(admin, DENOM);
console.log("Admin OSMO", (Number(balStart.amount) / 1e6).toFixed(3));

if (Number(balStart.amount) < 1_000_000) {
  try {
    await volunteerClient.sendTokens(
      volunteer,
      admin,
      coins(1_500_000, DENOM),
      fee,
      "volunteer tops up admin",
    );
    console.log("Volunteer sent 1.5 OSMO to admin");
  } catch (e) {
    console.log("Top-up skip", e.message?.slice(0, 80));
  }
}

const SKIP_DEPLOY = process.env.SKIP_DEPLOY === "1";

const addresses = {
  CitizenScienceContractAddress:
    process.env.CS_CONTRACT ??
    "osmo1nqqev3y5l8sjgrghuplagy0td3tdcy0cklx9mqnze27j2ynu7jrqram74j",
  CrossPlatformExchangeContractAddress:
    process.env.XC_CONTRACT ??
    "osmo1qxsrslwmzdexrra3jt7h3evftsf348t5lt9r0qj2lfrxf7cz646qnu7fgl",
  WaterWellContractAddress:
    process.env.WW_CONTRACT ??
    "osmo136w4meusng34dww46narwdmgpc5qqwfm9csneqhl2yv076fyfelqsue5vz",
  UtilityWaterFootprintContractAddress:
    process.env.WU_CONTRACT ??
    "osmo1ds3ua549g4unzyjv9t3xx2gger0hlkssl26sq0c3pnwsu6q7tcyqvp6wgn",
  SustainableActionRewardsContractAddress:
    process.env.SA_CONTRACT ??
    "osmo1n79372d8rcsusseddf73p4398tmx32aqz4aw7ma7dgq7yfkn5y0s8kfpq4",
  CommunityBountyContractAddress:
    process.env.CB_CONTRACT ??
    "osmo13egnf4w87frf5ee4txn888aw0vj24yj7dxckud6rveyts5lhetjq2yurn6",
  WaterCreditMarketplaceContractAddress:
    process.env.WC_CONTRACT ??
    "osmo1fn03cqru46u6upw9wcfwvq77jvmqtmuvpdgjlnk67yg8cg43q0gqknwd4u",
  LocalDaoContractAddress:
    process.env.DAO_CONTRACT ??
    "osmo143v798f900xv0z3j52f7vrdj8lf43ervnj62f3kx66vace85neuqvpuyg0",
};

// Redeploy thnl-admin modules under fa75
if (!SKIP_DEPLOY) {
  addresses.WaterWellContractAddress = await deploy(
  adminClient,
  admin,
  "water-well-initiative",
  "water_well_initiative.wasm",
);
addresses.UtilityWaterFootprintContractAddress = await deploy(
  adminClient,
  admin,
  "utility-water-footprint",
  "utility_water_footprint.wasm",
);
addresses.SustainableActionRewardsContractAddress = await deploy(
  adminClient,
  admin,
  "sustainable-action-rewards",
  "sustainable_action_rewards.wasm",
);
addresses.CommunityBountyContractAddress = await deploy(
  adminClient,
  admin,
  "community-bounty",
  "community_bounty.wasm",
);
addresses.WaterCreditMarketplaceContractAddress = await deploy(
  adminClient,
  admin,
  "water-credit-marketplace",
  "water_credit_marketplace.wasm",
);
addresses.LocalDaoContractAddress = await deploy(
  adminClient,
  admin,
  "local-dao",
  "local_dao.wasm",
  { quorum_bps: 3000, voting_period_seconds: 604800 },
  );
}

// Fund demo actors (skip if already funded)
if (!SKIP_DEPLOY) {
for (const [client, addr, name, osmo] of [
  [adminClient, volunteer, "volunteer", 2],
  [adminClient, buyer, "buyer", 2],
]) {
  try {
    await client.sendTokens(admin, addr, coins(osmo * 1_000_000, DENOM), fee, `fund ${name}`);
    console.log(`Funded ${name}`, osmo, "OSMO");
  } catch (e) {
    console.log(`Fund ${name} skip`, e.message?.slice(0, 80));
  }
}
}

// --- Citizen science (existing fa75 admin) ---
if (!SKIP_DEPLOY) {
const cs = addresses.CitizenScienceContractAddress;
try {
  await exec(adminClient, admin, cs, { add_verifier: { verifier: admin } }, "cs add verifier");
} catch {}
const csSensors = [
  {
    type: "Water Quality",
    model: "narmada-ahmedabad",
    location: { lat: "23.02", lng: "72.57", description: "Sabarmati tributary — Ahmedabad, Gujarat" },
  },
  {
    type: "Water Level",
    model: "musi-hyderabad",
    location: { lat: "17.38", lng: "78.47", description: "Musi river gauge — Hyderabad, Telangana" },
  },
];
const csIds = [];
for (const data of csSensors) {
  const res = await exec(adminClient, admin, cs, { submit_sensor: { data } }, `cs sensor ${data.model}`);
  csIds.push(Number(attr(res, "sensor_id")));
}
for (const id of csIds) {
  await exec(adminClient, admin, cs, { activate: { sensor_id: id } }, `cs activate ${id}`);
  const res = await exec(
    volunteerClient,
    volunteer,
    cs,
    { submit_data: { sensor_id: id, data: { value: "24.2", unit: "C" } } },
    `cs reading ${id}`,
  );
  const entryId = Number(attr(res, "entry_id"));
  if (entryId) {
    await exec(adminClient, admin, cs, { verify_data: { entry_id: entryId } }, `cs verify ${entryId}`);
  }
}
}

// --- Water well ---
const ww = addresses.WaterWellContractAddress;
const wwProjects = [
  {
    goal: "5000000",
    data: {
      title: "Borewell revival — Nashik grape belt",
      location: "Nashik, Maharashtra, India",
      description: "Deepening and solar pump for tribal hamlets near Godavari basin.",
    },
    validate: true,
    donate: "1500000",
  },
  {
    goal: "3200000",
    data: {
      title: "School rainwater tanks — Pune",
      location: "Pune, Maharashtra, India",
      description: "Roof catchment for three municipal schools in Kothrud.",
    },
    validate: true,
    donate: "800000",
  },
];
for (const p of wwProjects) {
  const res = await exec(adminClient, admin, ww, { create_project: { goal: p.goal, data: p.data } }, p.data.title);
  const pid = res ? Number(attr(res, "project_id")) : null;
  if (!pid) continue;
  if (p.validate) {
    await exec(adminClient, admin, ww, { validate: { project_id: pid } }, `ww validate ${pid}`);
    await exec(
      adminClient,
      buyer,
      ww,
      { donate: { project_id: pid } },
      `ww donate ${pid}`,
      [coin(Number(p.donate), DENOM)],
    );
  }
}

// --- Water utilities ---
const wu = addresses.UtilityWaterFootprintContractAddress;
await exec(adminClient, admin, wu, { add_verifier: { verifier: admin } }, "wu verifier");
const utilCos = [
  {
    name: "Chennai Metro Water",
    metadata: { sector: "municipal", region: "Chennai, Tamil Nadu, India" },
    logs: [
      { period: "2026-Q1", usage: "880000", savings: "110000" },
      { period: "2026-Q2", usage: "820000", savings: "95000" },
    ],
  },
  {
    name: "Surat Municipal Corp",
    metadata: { sector: "municipal", region: "Surat, Gujarat, India" },
    logs: [{ period: "2026-H1", usage: "640000", savings: "88000" }],
  },
];
for (const co of utilCos) {
  const reg = await exec(
    adminClient,
    admin,
    wu,
    { register_company: { name: co.name, metadata: co.metadata } },
    `wu ${co.name}`,
  );
  const cid = Number(attr(reg, "company_id"));
  for (const log of co.logs) {
    const lr = await exec(
      adminClient,
      admin,
      wu,
      {
        log_usage: {
          company_id: cid,
          period: log.period,
          usage: log.usage,
          savings: log.savings,
        },
      },
      `wu log ${co.name} ${log.period}`,
    );
    const lid = Number(attr(lr, "log_id"));
    await exec(adminClient, admin, wu, { validate_log: { log_id: lid } }, `wu validate ${lid}`);
  }
  await exec(
    adminClient,
    admin,
    wu,
    { issue_certificate: { company_id: cid, period: co.logs[0].period } },
    `wu cert ${co.name}`,
  );
}

// --- Sustainable actions ---
const sa = addresses.SustainableActionRewardsContractAddress;
await exec(adminClient, admin, sa, { add_verifier: { verifier: admin } }, "sa verifier");
const actions = [
  {
    title: "Neem tree belt — Aravalli, Jaipur",
    location: "Jaipur, Rajasthan, India",
    description: "Planted 120 native trees along dry stream bed.",
    impact_points: "95",
    verify: true,
    reward: "75000",
  },
  {
    title: "Community soak pit — Kochi",
    location: "Kochi, Kerala, India",
    description: "Built recharge pits in Fort Kochi ward 12.",
    impact_points: "70",
    verify: true,
    reward: "50000",
  },
];
for (const a of actions) {
  const res = await exec(
    volunteerClient,
    volunteer,
    sa,
    {
      submit_action: {
        evidence: {
          title: a.title,
          location: a.location,
          description: a.description,
          impact_points: a.impact_points,
        },
      },
    },
    a.title,
  );
  const aid = Number(attr(res, "action_id"));
  if (a.verify && aid) {
    await exec(adminClient, admin, sa, { verify_action: { action_id: aid } }, `sa verify ${aid}`);
    if (a.reward) {
      await exec(
        adminClient,
        admin,
        sa,
        { reward_actor: { action_id: aid } },
        `sa reward ${aid}`,
        [coin(Number(a.reward), DENOM)],
      );
    }
  }
}

// --- Community bounty ---
const cb = addresses.CommunityBountyContractAddress;
const now = Math.floor(Date.now() / 1000);
const bounties = [
  {
    title: "Vellayambalam canal desilt — Thiruvananthapuram",
    location: "Thiruvananthapuram, Kerala, India",
    reward: 450000,
    deadline: now + 10 * 86400,
  },
  {
    title: "Ganga ghat cleanup — Varanasi",
    location: "Varanasi, Uttar Pradesh, India",
    reward: 600000,
    deadline: now + 14 * 86400,
  },
];
const bountyIds = [];
for (const b of bounties) {
  const res = await exec(
    adminClient,
    admin,
    cb,
    {
      post_bounty: {
        title: b.title,
        description: "On-chain escrow demo task with photo evidence required.",
        location: b.location,
        deadline: b.deadline,
      },
    },
    b.title,
    [coin(b.reward, DENOM)],
  );
  bountyIds.push(Number(attr(res, "bounty_id")));
}
if (bountyIds[0]) {
  const sub = await exec(
    volunteerClient,
    volunteer,
    cb,
    {
      submit_work: {
        bounty_id: bountyIds[0],
        work: {
          summary: "Desilted 200 m canal section, 42 kg debris logged",
          location: "Thiruvananthapuram, Kerala, India",
          evidence: "Ward engineer sign-off hash",
          hours_spent: "5",
        },
      },
    },
    "bounty submit canal",
  );
  const sid = Number(attr(sub, "submission_id"));
  if (sid) {
    await exec(
      adminClient,
      admin,
      cb,
      { approve_work: { bounty_id: bountyIds[0], submission_id: sid } },
      "bounty approve",
    );
  }
}

// --- Water credits ---
const wc = addresses.WaterCreditMarketplaceContractAddress;
await exec(adminClient, admin, wc, { mint_credits: { recipient: admin, amount: "1000" } }, "wc mint admin");
await exec(adminClient, admin, wc, { mint_credits: { recipient: volunteer, amount: "300" } }, "wc mint volunteer");
const exp = now + 14 * 86400;
const list1 = await exec(
  adminClient,
  admin,
  wc,
  {
    list_credit: {
      credits: "150",
      price: "500000",
      region: "Krishna delta irrigation district, Andhra Pradesh, India",
      expires_at: exp,
    },
  },
  "wc list Andhra",
);
const list2 = await exec(
  volunteerClient,
  volunteer,
  wc,
  {
    list_credit: {
      credits: "80",
      price: "300000",
      region: "Coimbatore textile belt greywater savings, India",
      expires_at: exp,
    },
  },
  "wc list Coimbatore",
);
const lid = Number(attr(list2, "listing_id"));
if (lid) {
  await exec(
    buyerClient,
    buyer,
    wc,
    { buy_credit: { listing_id: lid } },
    "wc buy listing",
    [coin(300_000, DENOM)],
  );
}

// --- Local DAO ---
const dao = addresses.LocalDaoContractAddress;
const proposals = [
  {
    title: "Smart meters for Pune cantonment wards",
    description: "Install IoT flow meters on community standposts.",
    action_tag: "deploy_meters",
    metadata: { location: "Pune, Maharashtra, India", summary: "48 standposts" },
  },
  {
    title: "Restore Dal Lake floating gardens",
    description: "Fund native lotus bed restoration with weekly audits.",
    action_tag: "fund_restoration",
    metadata: { location: "Srinagar, Jammu & Kashmir, India", summary: "NGO partnership" },
  },
];
const propIds = [];
for (const p of proposals) {
  const res = await exec(
    adminClient,
    admin,
    dao,
    {
      create_proposal: {
        title: p.title,
        description: p.description,
        action_tag: p.action_tag,
        metadata: p.metadata,
      },
    },
    p.title,
  );
  propIds.push(Number(attr(res, "proposal_id")));
}
if (propIds[0]) {
  await exec(volunteerClient, volunteer, dao, { vote: { proposal_id: propIds[0], vote: { yes: {} } } }, "dao vote yes");
  await exec(buyerClient, buyer, dao, { vote: { proposal_id: propIds[0], vote: { yes: {} } } }, "dao vote yes 2");
}

// --- Cross exchange ---
process.env.CONTRACT = addresses.CrossPlatformExchangeContractAddress;
execSync("node seed-cross-exchange-osmosis.mjs", {
  cwd: resolve(dirname(fileURLToPath(import.meta.url))),
  stdio: "inherit",
  env: { ...process.env, DEMO_SWAP_OSMO: "2" },
});

try {
  await exec(
    buyerClient,
    buyer,
    addresses.CrossPlatformExchangeContractAddress,
    {
      swap: {
        partner_denom: "gujarat-water-unit",
        direction: { base_to_partner: {} },
        amount: "1000000",
      },
    },
    "buyer swap 1 OSMO → Gujarat",
    [coin(1_000_000, DENOM)],
  );
} catch (e) {
  console.log("buyer swap skip", e.message?.slice(0, 100));
}

// Update environment.prod.ts
let prod = readFileSync(PROD_ENV, "utf8");
for (const [key, addr] of Object.entries(addresses)) {
  const re = new RegExp(`${key}:\\s*\\n\\s*'[^']+'`);
  prod = prod.replace(re, `${key}:\n    '${addr}'`);
}
writeFileSync(PROD_ENV, prod);
console.log("\n=== Updated environment.prod.ts ===");

// Append demo actors to .env if missing
const envPath = resolve(ROOT, ".env");
let envText = readFileSync(envPath, "utf8");
if (!envText.includes("DEMO_VOLUNTEER_ADDRESS")) {
  envText += `\nDEMO_VOLUNTEER_ADDRESS=${volunteer}\nDEMO_BUYER_ADDRESS=${buyer}\n`;
  writeFileSync(envPath, envText);
}

const balEnd = await adminClient.getBalance(admin, DENOM);
console.log("Admin OSMO left", (Number(balEnd.amount) / 1e6).toFixed(3));
console.log("\nAddresses:", JSON.stringify(addresses, null, 2));
