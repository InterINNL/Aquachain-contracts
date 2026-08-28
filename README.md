# AquaChain contracts

CosmWasm (Sylvia) smart contracts for AquaChain water-management demos.

| Contract                     | Crate                      | Role                                           |
| ---------------------------- | -------------------------- | ---------------------------------------------- |
| **citizen-science-registry** | `citizen_science_registry` | Sensors, data submissions, verifiers, rewards  |
| **water-well-initiative**    | `water_well_initiative`    | Funded water projects, donations, disbursement |
| **utility-water-footprint**  | `utility_water_footprint`  | Utility companies, usage logs, certificates    |
| **sustainable-action-rewards** | `sustainable_action_rewards` | Community eco actions, verify, reward impact |
| **community-bounty** | `community_bounty` | Escrowed sustainability tasks, submissions, payout |
| **water-credit-marketplace** | `water_credit_marketplace` | Internal conservation credit ledger and marketplace |
| **local-dao** | `local_dao` | Community proposals, voting, and on-chain finalization |
| **cross-platform-exchange** | `cross_platform_exchange` | Fixed-rate swaps between base denom and partner ledgers |

Frontend: [InterINNL/Aquachain-frontend](https://github.com/InterINNL/Aquachain-frontend)

## Requirements

- Rust nightly + `wasm32-unknown-unknown` (builds use `-Zbuild-std`)
- [`wasmd`](https://github.com/CosmWasm/wasmd) on `PATH` (local: `chain-id=testing`, denom `ustake`, prefix `wasm`)
- `jq`

## Build and test

```sh
cd contracts/citizen-science-registry   # or water-well-initiative / utility-water-footprint
make test
make build
make schema
```

WASM output:

`target/wasm32-unknown-unknown/release/<crate>.wasm`

## Local deploy (wasmd)

Defaults in the contract Makefiles:

| Variable   | Default                  |
| ---------- | ------------------------ |
| `CHAIN_ID` | `testing`                |
| `NODE`     | `http://localhost:26657` |
| `DENOM`    | `ustake`                 |
| `KEY_NAME` | `greg`                   |

```sh
# Start wasmd (Docker example from CosmWasm wasmd; fund a wasm1… Keplr address)
docker volume rm -f wasmd_data
docker run --rm -it \
  -e PASSWORD=xxxxxxxxx \
  --mount type=volume,source=wasmd_data,target=/root \
  cosmwasm/wasmd:latest /opt/setup_wasmd.sh wasm1YOUR_ADDRESS

docker run --rm -it -p 26657:26657 -p 26656:26656 -p 1317:1317 \
  --mount type=volume,source=wasmd_data,target=/root \
  cosmwasm/wasmd:latest /opt/run_wasmd.sh

cd contracts/citizen-science-registry
make deploy
# address → contract_addr.txt ; paste into frontend CitizenScienceContractAddress

cd ../water-well-initiative
make deploy
# address → contract_addr.txt ; paste into frontend WaterWellContractAddress

cd ../utility-water-footprint
make deploy
# address → contract_addr.txt ; paste into frontend UtilityWaterFootprintContractAddress
```

Override network when needed:

```sh
make deploy CHAIN_ID=… NODE=… DENOM=… KEY_NAME=…
```

## Osmosis testnet deploy (`osmo-test-5`)

Requires sibling frontend `node_modules` (`@cosmjs/*`). Auth: set `MNEMONIC` or `PRIVATE_KEY` (32-byte hex) for an `osmo1…` account with testnet OSMO.

Build each crate first (`make build`), then from the contracts repo root:

| Order | Label env                        | WASM artifact                                                         | Frontend env key                       |
| ----- | -------------------------------- | --------------------------------------------------------------------- | -------------------------------------- |
| 1     | `LABEL=citizen-science-registry` | `target/wasm32-unknown-unknown/release/citizen_science_registry.wasm` | `CitizenScienceContractAddress`        |
| 2     | `LABEL=water-well-initiative`    | `…/water_well_initiative.wasm`                                        | `WaterWellContractAddress`             |
| 3     | `LABEL=utility-water-footprint`  | `…/utility_water_footprint.wasm`                                      | `UtilityWaterFootprintContractAddress` |
| 4     | `LABEL=sustainable-action-rewards` | `…/sustainable_action_rewards.wasm`                                 | `SustainableActionRewardsContractAddress` |
| 5     | `LABEL=community-bounty` | `…/community_bounty.wasm` | `CommunityBountyContractAddress` |
| 6     | `LABEL=water-credit-marketplace` | `…/water_credit_marketplace.wasm` | `WaterCreditMarketplaceContractAddress` |
| 7     | `LABEL=local-dao` | `…/local_dao.wasm` | `LocalDaoContractAddress` |
| 8     | `LABEL=cross-platform-exchange` | `…/cross_platform_exchange.wasm` | `CrossPlatformExchangeContractAddress` |

```sh
LABEL=cross-platform-exchange node scripts/deploy-osmosis.mjs \
  target/wasm32-unknown-unknown/release/cross_platform_exchange.wasm
```

Paste each printed `contract` address into frontend `environment.prod.ts` (live) or `environment.ts` (local).

Optional demo seeds (same auth env vars; optional `CONTRACT=osmo1…`):

| Script                                | Module          |
| ------------------------------------- | --------------- |
| `scripts/seed-osmosis.mjs`            | Citizen Science |
| `scripts/seed-water-well-osmosis.mjs` | Water Well      |
| `scripts/seed-footprint-osmosis.mjs`  | Water Utilities |
| `scripts/seed-sustainable-actions-osmosis.mjs` | Sustainable Actions |
| `scripts/seed-community-bounty-osmosis.mjs` | Community Bounty |
| `scripts/seed-water-credits-osmosis.mjs` | Water Credits |
| `scripts/seed-local-dao-osmosis.mjs` | Local DAO |
| `scripts/seed-cross-exchange-osmosis.mjs` | Cross Exchange |

## Demo checklist (eight modules)

1. Deploy all eight contracts (local wasmd **or** Osmosis as above).
2. Set the eight env keys in the frontend build that reviewers will open.
3. Connect Keplr to the matching chain; fund the account.
4. Smoke each path:
   - **Citizen Science:** register sensor → activate → submit data → verify → reward
   - **Water Well:** create → validate → donate → unlock → disburse
   - **Water Utilities:** register company → log usage/savings → validate → issue certificate (≥10% validated savings ratio)
   - **Sustainable Actions:** submit action → verify → reward with funds
   - **Community Bounty:** post bounty with escrow → submit work → approve winner (payout)
   - **Water Credits:** mint credits → list for sale → buy listing (atomic pay + transfer)
   - **Local DAO:** create proposal → vote → execute after voting period (quorum + yes majority)
   - **Cross Exchange:** register partner + rate (admin seed) → swap base→partner → swap partner→base or withdraw locked units

Demo geography: use **Indian cities and regions** in seed scripts and UI examples (Delhi, Bengaluru, Udaipur, Mumbai, Gujarat, etc.).

Live demo: [interinnl.interchouette.net](https://interinnl.interchouette.net)

## License

MIT
