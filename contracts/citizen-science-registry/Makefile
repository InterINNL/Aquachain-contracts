MAKEFLAGS += --no-print-directory

build:
	RUSTFLAGS="-C target-cpu=mvp" cargo +nightly build -Zbuild-std=panic_abort,std \
		--target wasm32-unknown-unknown --release --lib

# Generate JSON schema definitions
schema:
	cargo run --bin schema

test:
	cargo test

deploy: store install
	@echo "Contract deployed and instantiated successfully."

KEY_NAME=greg
CHAIN_ID=testing
NODE=http://localhost:26657
DENOM=ustake
WASM_PATH=../../target/wasm32-unknown-unknown/release/citizen_science_registry.wasm

store:
	@echo "Storing wasm..."
	@TX_HASH=$$( \
		wasmd tx wasm store ${WASM_PATH} \
			--from $(KEY_NAME) \
			--gas auto --gas-adjustment 1.3 \
			--chain-id $(CHAIN_ID) --node $(NODE) \
			--fees 5000$(DENOM) \
			--keyring-backend file \
			--broadcast-mode=sync -y | \
		tee /dev/stderr | grep -oE 'txhash: [0-9A-F]+' | awk '{print $$2}' \
	); \
	echo "Waiting for transaction...$$TX_HASH"; \
	sleep 5; \
	CODE_ID=$$( \
		wasmd query wasm list-code --node $(NODE) --output json | jq -r '.code_infos | last.code_id' \
	); \
	echo $$CODE_ID > code_id.txt; \
	echo "Stored successfully. CODE_ID=$$CODE_ID"

install:
	@echo "Instantiating contract with CODE_ID from code_id.txt..."
	@CODE_ID=$$(cat code_id.txt); \
	ADDR=$$(wasmd keys show $(KEY_NAME) -a --keyring-backend file); \
	INIT_MSG='{"denom":"$(DENOM)"}'; \
	echo "Using CODE_ID=$$CODE_ID, ADMIN=$$ADDR, INIT_MSG=$$INIT_MSG"; \
	TX_HASH=$$( \
	wasmd tx wasm instantiate $$CODE_ID "$$INIT_MSG" \
		--from $(KEY_NAME) \
		--label "citizen-science-registry" \
		--admin $$ADDR \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000$(DENOM) \
		--keyring-backend file \
		--broadcast-mode=sync -y | \
		tee /dev/stderr | grep -oE 'txhash: [0-9A-F]+' | awk '{print $$2}' \
	); \
	echo "Waiting for transaction...$$TX_HASH"; \
	sleep 20; \
	CONTRACT_ADDR=$$(wasmd query tx $$TX_HASH --node $(NODE) --output json | jq -r '.events[]? | select(.type=="instantiate") | .attributes[]? | select(.key=="_contract_address") | .value'); \
	echo "CONTRACT_ADDR=$$CONTRACT_ADDR"; \
	echo $$CONTRACT_ADDR > contract_addr.txt

submit-sensor:
	@DATA='$(data)'; \
	if [ -z "$$DATA" ]; then echo "Usage: make submit-sensor data='{\"key\":\"value\"}'"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	MSG=$$(jq -n --arg data "$$DATA" '{ submit_sensor: { data: ($$data | fromjson) } }'); \
	echo "Sending execute message: $$MSG"; \
	TX_HASH=$$( \
		wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
			--from $(KEY_NAME) \
			--gas auto --gas-adjustment 1.3 \
			--chain-id $(CHAIN_ID) --node $(NODE) \
			--fees 5000$(DENOM) \
			--keyring-backend file \
			--broadcast-mode=sync -y | \
		tee /dev/stderr | grep -oE 'txhash: [0-9A-F]+' | awk '{print $$2}' \
	); \
	echo "Waiting for transaction... $$TX_HASH"; \
	sleep 5; \
	wasmd query tx $$TX_HASH --node $(NODE)

activate:
	@SENSOR_ID=$(sensor_id); \
	if [ -z "$$SENSOR_ID" ]; then echo "Usage: make activate sensor_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	MSG=$$(jq -nc --argjson id "$$SENSOR_ID" '{ activate: { sensor_id: $$id } }'); \
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000$(DENOM) \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

deactivate:
	@SENSOR_ID=$(sensor_id); \
	if [ -z "$$SENSOR_ID" ]; then echo "Usage: make deactivate sensor_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	MSG=$$(jq -nc --argjson id "$$SENSOR_ID" '{ deactivate: { sensor_id: $$id } }'); \
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000$(DENOM) \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

get_sensor:
	@SENSOR_ID=$(sensor_id); \
	if [ -z "$$SENSOR_ID" ]; then echo "Usage: make get_sensor sensor_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying sensor id: $$SENSOR_ID from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --argjson sensor_id "$$SENSOR_ID" '{ get_sensor: { sensor_id: $$sensor_id } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

list_sensors:
	@START_AFTER=$${start_after:-null}; \
	LIMIT=$${limit:-10}; \
	if [ "$$START_AFTER" = "null" ]; then echo "ℹUsage: make list_sensors start_after=0 limit=10 (defaults: start_after=null, limit=10)"; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "📄 Listing sensors from contract $$CONTRACT_ADDR with start_after=$$START_AFTER and limit=$$LIMIT"; \
	QUERY_MSG=$$(jq -nc \
		--argjson start_after "$$START_AFTER" \
		--argjson limit "$$LIMIT" \
		'{ list_sensors: { start_after: $$start_after, limit: $$limit } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

add-verifier:
	@echo "Adding verifier..."
	@VERIFIER=$$verifier; \
	if [ -z "$$VERIFIER" ]; then echo "Usage: make add-verifier verifier=wasm1..."; exit 1; fi; \
	CONTRACT=$$(cat contract_addr.txt); \
	MSG=$$(jq -nc --arg verifier "$$VERIFIER" '{ "add_verifier": { "verifier": $$verifier } }'); \
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000$(DENOM) \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

list-verifiers:
	@echo "Usage: make list-verifiers"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying list_verifiers from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc '{ list_verifiers: {} }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

is-verifier:
	@echo "Is verifier..."
	@VERIFIER=$$verifier; \
	if [ -z "$$VERIFIER" ]; then echo "Usage: make is-verifier verifier=wasm1..."; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying is_verifier for address $$VERIFIER from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --arg verifier "$$VERIFIER" '{ is_verifier: { verifier: $$verifier } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

get-data-entry:
	@ENTRY_ID=$(entry_id); \
	if [ -z "$$ENTRY_ID" ]; then echo "Usage: make get-data-entry entry_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying data entry id: $$ENTRY_ID from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --argjson entry_id "$$ENTRY_ID" '{ get_data_entry: { entry_id: $$entry_id } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

list-data-entries:
	@echo "ℹ️  Usage: make list-data-entries start_after=<u64> limit=<u32>"; \
	START_AFTER=$(start_after); \
	LIMIT=$(limit); \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Listing data entries from contract $$CONTRACT_ADDR with start_after=$$START_AFTER and limit=$$LIMIT"; \
	QUERY_MSG=$$(jq -nc \
		--argjson start_after "$${START_AFTER:-null}" \
		--argjson limit "$${LIMIT:-null}" \
		'{ list_data_entries: { start_after: $$start_after, limit: $$limit } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

submit-data:
	@SENSOR_ID=$(sensor_id); \
	DATA=$(data); \
	if [ -z "$$SENSOR_ID" ]; then echo "Usage: make submit-data sensor_id=1 data='{\"key\":\"value\"}'"; exit 1; fi; \
	if [ -z "$$DATA" ]; then echo "Usage: make submit-data sensor_id=1 data='{\"key\":\"value\"}'"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	MSG=$$(jq -n --arg sensor_id "$$SENSOR_ID" --arg data "$$DATA" '{ submit_data: { sensor_id: ($$sensor_id|tonumber), data: $$data } }'); \
	echo "Sending execute message: $$MSG"; \
	TX_HASH=$$( \
		wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
			--from $(KEY_NAME) \
			--gas auto --gas-adjustment 1.3 \
			--chain-id $(CHAIN_ID) --node $(NODE) \
			--fees 5000$(DENOM) \
			--keyring-backend file \
			--broadcast-mode=sync -y | \
		tee /dev/stderr | grep -oE 'txhash: [0-9A-F]+' | awk '{print $$2}' \
	); \
	echo "Waiting for transaction... $$TX_HASH"; \
	sleep 5; \
	wasmd query tx $$TX_HASH --node $(NODE)

verify-data:
	@ENTRY_ID=$(entry_id); \
	if [ -z "$$ENTRY_ID" ]; then echo "Usage: make verify-data entry_id=1"; exit 1; fi; \
	MSG=$$(jq -n --argjson id "$$ENTRY_ID" '{ verify_data: { entry_id: $$id } }'); \
	echo "Sending execute message: $$MSG"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000ustake \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

reward-submitter:
	@ENTRY_ID=$(entry_id); \
	REWARD=$(reward); \
	MSG=$$(jq -n --argjson id "$$ENTRY_ID" '{ reward_submitter: { entry_id: $$id } }'); \
	echo "Sending execute message with entry_id=$${ENTRY_ID} and reward=$${REWARD}$(DENOM)"; \
	echo "$$MSG"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--amount "$${REWARD}$(DENOM)" \
		--gas auto --gas-adjustment 1.3 \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--fees 5000$(DENOM) \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

flow:
	@echo "Starting full contract flow test..."

	@echo "\n==> Store contract"
	$(MAKE) store || exit 1

	@echo "\n==> Install contract"
	$(MAKE) install || exit 1

	@echo "\n==> Submit new sensor"
	$(MAKE) submit-sensor data='{"type":"Water Quality","model":"test","location":{"lat":"89","lng":"98","description":"Downtown"}}' || exit 1

	@sleep 5
	@echo "\n==> Activate sensor 1"
	$(MAKE) activate sensor_id=1 || exit 1

	@sleep 5
	@echo "\n==> Get sensor 1"
	$(MAKE) get_sensor sensor_id=1 || exit 1

	@sleep 5
	@echo "\n==> List sensors"
	$(MAKE) list_sensors start_after=0 limit=5 || exit 1

	@sleep 5
	@echo "\n==> Add verifier"
	ADDR=$$(wasmd keys show $(KEY_NAME) -a --keyring-backend file); \
	$(MAKE) add-verifier verifier="$$ADDR" || exit 1

	@echo "\n==> Is verifier"
	ADDR=$$(wasmd keys show $(KEY_NAME) -a --keyring-backend file); \
	$(MAKE) is-verifier verifier="$$ADDR" || exit 1

	@echo "\n==> List verifiers"
	$(MAKE) list-verifiers || exit 1

	@echo "\n==> Submit data entry"
	sensor_id=1 data='{"pollution":15}' $(MAKE) submit-data || exit 1

	@sleep 5
	@echo "\n==> Query data entry 1"
	$(MAKE) get-data-entry entry_id=1 || exit 1

	@sleep 5
	@echo "\n==> List data entries"
	$(MAKE) list-data-entries start_after=0 limit=5 || exit 1

	@sleep 5
	@echo "\n==> Verify data entry 1"
	$(MAKE) verify-data entry_id=1 || exit 1

	@sleep 5
	@echo "\n==> Reward submitter for entry 1 with 1000"
	$(MAKE) reward-submitter entry_id=1 reward=1000 || exit 1

	@sleep 5
	@echo "\n==> Deactivate sensor 1"
	$(MAKE) deactivate sensor_id=1 || exit 1

	@echo "\nFull flow test completed."
