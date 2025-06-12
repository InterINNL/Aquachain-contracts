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

REWARD_AMOUNT=1000000

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

# Use extracted CODE_ID to instantiate
install:
	@echo "Instantiating contract with CODE_ID from code_id.txt..."
	@CODE_ID=$$(cat code_id.txt); \
	ADDR=$$(wasmd keys show $(KEY_NAME) -a --keyring-backend file); \
	echo "Using CODE_ID=$$CODE_ID and ADMIN=$$ADDR"; \
	TX_HASH=$$( \
	wasmd tx wasm instantiate $$CODE_ID '{}' \
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

add-verifier:
	@echo "Adding verifier..."
	@VERIFIER=$$verifier; \
	if [ -z "$$VERIFIER" ]; then echo "❌ Usage: make add-verifier verifier=wasm1..."; exit 1; fi; \
	CONTRACT=$$(cat contract_addr.txt); \
	MSG=$$(jq -nc --arg verifier "$$VERIFIER" '{ "add_verifier": { "verifier": $$verifier } }'); \
	echo "Sending: $$MSG"; \
	TX_HASH=$$( \
		wasmd tx wasm execute $$CONTRACT "$$MSG" \
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
	wasmd query tx $$TX_HASH --node $(NODE)

list-verifiers:
	@CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying list_verifiers from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc '{ list_verifiers: {} }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

is-verifier:
	@echo "Is verifier..."
	@VERIFIER=$$verifier; \
	if [ -z "$$VERIFIER" ]; then echo "❌ Usage: make is-verifier verifier=wasm1..."; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying is_verifier for address $$VERIFIER from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --arg verifier "$$VERIFIER" '{ is_verifier: { verifier: $$verifier } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

query-data-entry:
	@ENTRY_ID=$(entry_id); \
	if [ -z "$$ENTRY_ID" ]; then echo "❌ Usage: make query-data-entry entry_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying data entry id: $$ENTRY_ID from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --argjson entry_id "$$ENTRY_ID" '{ get_data_entry: { entry_id: $$entry_id } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

list-data-entries:
	@START_AFTER=$(start_after); \
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
	@DATA=$(data); \
	if [ -z "$$DATA" ]; then echo "❌ Usage: make submit-data data='{\"key\":\"value\"}'"; exit 1; fi; \
	MSG=$$(jq -nc --arg data "$$DATA" '{ submit_data: { data: $$data } }'); \
	echo "Sending execute message: $$MSG"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id testing --node http://localhost:26657 \
		--fees 5000ustake \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

verify-data:
	@ENTRY_ID=$(entry_id); \
	if [ -z "$$ENTRY_ID" ]; then echo "❌ Usage: make verify-data entry_id=1"; exit 1; fi; \
	MSG=$$(jq -n --argjson id "$$ENTRY_ID" '{ verify_data: { entry_id: $$id } }'); \
	echo "Sending execute message: $$MSG"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id testing --node http://localhost:26657 \
		--fees 5000ustake \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr

reward-contributor:
	@ENTRY_ID=$(entry_id); \
	if [ -z "$$ENTRY_ID" ]; then echo "❌ Usage: make reward-contributor entry_id=1"; exit 1; fi; \
	MSG=$$(jq -n --argjson id "$$ENTRY_ID" '{ reward_contributor: { entry_id: $$id } }'); \
	echo "Sending execute message: $$MSG"; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--amount $(REWARD_AMOUNT)$(DENOM) \
		--gas auto --gas-adjustment 1.3 \
		--chain-id testing --node http://localhost:26657 \
		--fees 5000ustake \
		--keyring-backend file \
		--broadcast-mode=sync -y | tee /dev/stderr