# AquaChain contracts

CosmWasm (Sylvia) smart contracts for AquaChain water-management demos.

| Contract                     | Crate                      | Role                                           |
| ---------------------------- | -------------------------- | ---------------------------------------------- |
| **citizen-science-registry** | `citizen_science_registry` | Sensors, data submissions, verifiers, rewards  |
| **water-well-initiative**    | `water_well_initiative`    | Funded water projects, donations, disbursement |
| **utility-water-footprint**  | `utility_water_footprint`  | Utility companies, usage logs, certificates    |

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

## License

MIT
