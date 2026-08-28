#!/usr/bin/env node
/**
 * Seed demo Utility Water Footprint activity on Osmosis testnet.
 *
 *   PRIVATE_KEY='…' CONTRACT=osmo1… node seed-footprint-osmosis.mjs
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

const RPC = process.env.RPC ?? "https://rpc.osmotest5.osmosis.zone";
const CONTRACT =
  process.env.CONTRACT ??
  "osmo1j92tmc5d2vvrrar8krmr44v9zk2jw7fw8em0fekeqqd8la44quls3zmz5z";

const fee = {
  amount: [{ denom: "uosmo", amount: process.env.FEE_AMOUNT ?? "80000" }],
  gas: "1500000",
};


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

function attr(res, key) {
  const a = res.events.flatMap((e) => e.attributes).find((x) => x.key === key);
  return a?.value;
}

console.log("Signer  ", me);
console.log("Contract", CONTRACT);

try {
  await exec({ add_verifier: { verifier: me } }, "add self as verifier");
} catch (e) {
  console.log("  (verifier)", e.message?.slice(0, 120) ?? e);
}

const companies = [
  {
    name: "Delhi Jal Board",
    metadata: {
      sector: "municipal utility",
      region: "Delhi NCR, India",
      notes: "Yamuna basin supply and treatment footprint",
    },
    logs: [
      { period: "2026-Q1", usage: "1000000", savings: "150000" },
      { period: "2026-Q2", usage: "900000", savings: "120000" },
    ],
    certify: "2026-Q1",
  },
  {
    name: "BWSSB Bengaluru",
    metadata: {
      sector: "municipal utility",
      region: "Karnataka, India",
      notes: "Cauvery stage-4 distribution savings program",
    },
    logs: [{ period: "2026-H1", usage: "500000", savings: "80000" }],
    certify: null,
  },
  {
    name: "Chennai Metro Water",
    metadata: {
      sector: "municipal utility",
      region: "Chennai, Tamil Nadu, India",
      notes: "Desalination and STP reuse credits",
    },
    logs: [
      { period: "2026-Q1", usage: "880000", savings: "110000" },
      { period: "2026-Q2", usage: "820000", savings: "95000" },
    ],
    certify: "2026-Q1",
  },
  {
    name: "Surat Municipal Corporation",
    metadata: {
      sector: "municipal utility",
      region: "Surat, Gujarat, India",
      notes: "Tapi river intake efficiency upgrades",
    },
    logs: [{ period: "2026-H1", usage: "640000", savings: "88000" }],
    certify: "2026-H1",
  },
];

for (const co of companies) {
  const reg = await exec(
    { register_company: { name: co.name, metadata: co.metadata } },
    `register ${co.name}`,
  );
  const companyId = Number(attr(reg, "company_id"));
  console.log("  company_id", companyId);

  const logIds = [];
  for (const log of co.logs) {
    const res = await exec(
      {
        log_usage: {
          company_id: companyId,
          period: log.period,
          usage: log.usage,
          savings: log.savings,
        },
      },
      `log ${co.name} ${log.period}`,
    );
    const logId = Number(attr(res, "log_id"));
    logIds.push(logId);
    console.log("  log_id", logId);
  }

  // Validate enough logs for certificate (admin can validate)
  for (const logId of logIds) {
    if (!Number.isFinite(logId)) continue;
    try {
      await exec({ validate_log: { log_id: logId } }, `validate ${logId}`);
    } catch (e) {
      console.log("  (validate)", e.message?.slice(0, 120) ?? e);
    }
  }

  if (co.certify) {
    try {
      await exec(
        {
          issue_certificate: {
            company_id: companyId,
            period: co.certify,
          },
        },
        `issue cert ${co.name} ${co.certify}`,
      );
    } catch (e) {
      console.log("  (cert)", e.message?.slice(0, 160) ?? e);
    }
  }
}

const listed = await query({
  list_companies: { start_after: null, limit: 20 },
});
const logs = await query({
  list_logs: { company_id: null, start_after: null, limit: 30 },
});
const certs = await query({
  list_certificates: { company_id: null, start_after: null, limit: 20 },
});

console.log(
  "Companies",
  listed.map((c) => ({ id: c.id, name: c.name })),
);
console.log(
  "Logs",
  logs.map((l) => ({
    id: l.id,
    company_id: l.company_id,
    period: l.period,
    validated: l.validated,
  })),
);
console.log(
  "Certificates",
  certs.map((c) => ({
    id: c.id,
    company_id: c.company_id,
    period: c.period,
    savings: c.total_savings,
  })),
);
