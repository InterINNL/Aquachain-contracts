build:
	RUSTFLAGS="-C target-cpu=mvp" cargo +nightly build -Zbuild-std=panic_abort,std \
		--target wasm32-unknown-unknown --release --lib

schema:
	cargo run --bin schema

test:
	cargo test

KEY_NAME=greg
CHAIN_ID=testing
NODE=http://localhost:26657
DENOM=ustake
WASM_PATH=../../target/wasm32-unknown-unknown/release/water_well_donation.wasm

deploy: store install
	@echo "Contract deployed and instantiated successfully."

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
		--label "water-well-donation" \
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

create:
	@echo "Creating project..."
	@GOAL=$(goal); \
	if [ -z "$$GOAL" ]; then echo "❌ Usage: make create goal=10000"; exit 1; fi; \
	CONTRACT=$$(cat contract_addr.txt); \
	MSG=$$(jq -nc --arg goal "$$GOAL" '{ "create_project": { "goal": ($$goal | tostring) } }'); \
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

query_project:
	@PROJECT_ID=$(project_id); \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying project id: $$PROJECT_ID from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --argjson project_id "$$PROJECT_ID" '{ get_project: { project_id: $$project_id } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

list_projects:
	@START_AFTER=$(start_after); \
	LIMIT=$(limit); \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Listing projects from contract $$CONTRACT_ADDR with start_after=$$START_AFTER and limit=$$LIMIT"; \
	QUERY_MSG=$$(jq -nc \
		--argjson start_after "$${START_AFTER:-null}" \
		--argjson limit "$${LIMIT:-null}" \
		'{ list_projects: { start_after: $$start_after, limit: $$limit } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

donate:
	@PROJECT_ID=$(project_id); \
	AMOUNT=$(amount); \
	if [ -z "$$PROJECT_ID" ] || [ -z "$$AMOUNT" ]; then \
		echo "Usage: make donate project_id=1 amount=1000000"; \
		exit 1; \
	fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Donating $$AMOUNT ustake to project $$PROJECT_ID on contract $$CONTRACT_ADDR"; \
	MSG=$$(jq -nc --argjson id "$$PROJECT_ID" '{ donate: { project_id: $$id } }'); \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--amount "$$AMOUNT"ustake \
		--gas auto --gas-adjustment 1.3 \
		--fees 5000$(DENOM) \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--keyring-backend file \
		--broadcast-mode=sync -y

disburse:
	@PROJECT_ID=$(project_id); \
	if [ -z "$$PROJECT_ID" ]; then \
		echo "Usage: make disburse project_id=1"; \
		exit 1; \
	fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Disbursing project $$PROJECT_ID on contract $$CONTRACT_ADDR"; \
	MSG=$$(jq -nc --argjson pid "$$PROJECT_ID" '{ disburse: { project_id: $$pid } }'); \
	echo "Sending: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--fees 5000$(DENOM) \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--keyring-backend file \
		--broadcast-mode=sync -y