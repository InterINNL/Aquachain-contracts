MAKEFLAGS += --no-print-directory

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
WASM_PATH=../../target/wasm32-unknown-unknown/release/water_well_initiative.wasm

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

install:
	@echo "Instantiating contract with CODE_ID from code_id.txt..."
	@CODE_ID=$$(cat code_id.txt); \
	ADDR=$$(wasmd keys show $(KEY_NAME) -a --keyring-backend file); \
	INIT_MSG='{"denom":"$(DENOM)"}'; \
	echo "Using CODE_ID=$$CODE_ID, ADMIN=$$ADDR, INIT_MSG=$$INIT_MSG"; \
	TX_HASH=$$( \
	wasmd tx wasm instantiate $$CODE_ID "$$INIT_MSG" \
		--from $(KEY_NAME) \
		--label "water-well-initiative" \
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
	@GOAL=$(goal); \
	DATA=$(data); \
	if [ -z "$$GOAL" ]; then echo "Usage: make create goal=10000 data='{\"key\":\"value\"}'"; exit 1; fi; \
	if [ -z "$$DATA" ]; then echo "Usage: make create goal=10000 data='{\"key\":\"value\"}'"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	MSG=$$(jq -n --arg data "$$DATA" --arg goal "$$GOAL" '{ create_project: { goal: $$goal, data: $$data } }'); \
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
	echo "Waiting for transaction...$$TX_HASH"; \
	sleep 5; \
	wasmd query tx $$TX_HASH --node $(NODE)

get_project:
	@PROJECT_ID=$(project_id); \
	if [ -z "$$PROJECT_ID" ]; then echo "Usage: make get_project project_id=1"; exit 1; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying project id: $$PROJECT_ID from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -nc --argjson project_id "$$PROJECT_ID" '{ get_project: { project_id: $$project_id } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

list_projects:
	@START_AFTER=$${start_after:-null}; \
	LIMIT=$${limit:-10}; \
	if [ "$$START_AFTER" = "null" ]; then echo "ℹUsage: make list_projects start_after=0 limit=10 (defaults: start_after=null, limit=10)"; fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "📄 Listing projects from contract $$CONTRACT_ADDR with start_after=$$START_AFTER and limit=$$LIMIT"; \
	QUERY_MSG=$$(jq -nc \
		--argjson start_after "$$START_AFTER" \
		--argjson limit "$$LIMIT" \
		'{ list_projects: { start_after: $$start_after, limit: $$limit } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

query_projects_by_status:
	@STATUS=$(status); \
	LIMIT=$(limit); \
	START_AFTER=$(start_after); \
	if [ -z "$$STATUS" ]; then \
		echo "Usage: make query_projects_by_status status=fundraising [limit=10] [start_after=0]"; \
		exit 1; \
	fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Querying projects with status: $$STATUS, limit: $${LIMIT:-10}, start_after: $${START_AFTER:-null} from contract $$CONTRACT_ADDR"; \
	QUERY_MSG=$$(jq -n \
		--arg status "$$STATUS" \
		--argjson limit "$${LIMIT:-null}" \
		--argjson start_after "$${START_AFTER:-null}" \
		'{ get_projects_by_status: { status: $$status, limit: $$limit, start_after: $$start_after } }'); \
	echo "Sending query: $$QUERY_MSG"; \
	wasmd query wasm contract-state smart $$CONTRACT_ADDR "$$QUERY_MSG" --node $(NODE) --output json

validate:
	@PROJECT_ID=$(project_id); \
	if [ -z "$$PROJECT_ID" ]; then \
		echo "Usage: make validate project_id=1"; \
		exit 1; \
	fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Validating project $$PROJECT_ID on contract $$CONTRACT_ADDR"; \
	MSG=$$(jq -nc --argjson pid "$$PROJECT_ID" '{ validate: { project_id: $$pid } }'); \
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--fees 5000$(DENOM) \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--keyring-backend file \
		--broadcast-mode=sync -y

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

unlock:
	@PROJECT_ID=$(project_id); \
	if [ -z "$$PROJECT_ID" ]; then \
		echo "Usage: make unlock project_id=1"; \
		exit 1; \
	fi; \
	CONTRACT_ADDR=$$(cat contract_addr.txt); \
	echo "Unlocking project $$PROJECT_ID on contract $$CONTRACT_ADDR"; \
	MSG=$$(jq -nc --argjson pid "$$PROJECT_ID" '{ unlock: { project_id: $$pid } }'); \
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
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
	echo "Sending execute message: $$MSG"; \
	wasmd tx wasm execute $$CONTRACT_ADDR "$$MSG" \
		--from $(KEY_NAME) \
		--gas auto --gas-adjustment 1.3 \
		--fees 5000$(DENOM) \
		--chain-id $(CHAIN_ID) --node $(NODE) \
		--keyring-backend file \
		--broadcast-mode=sync -y

flow:
	@echo "Starting full contract flow test..."

	@echo "\n==> Store contract"
	$(MAKE) store || exit 1

	@echo "\n==> Install contract"
	$(MAKE) install || exit 1

	@echo "\n==> Create project"
	goal=10000 data='{"description":"Test project","creator":"tester"}' $(MAKE) create || exit 1

	@sleep 5
	@echo "\n==> Validate project 1"
	$(MAKE) validate project_id=1 || exit 1

	@sleep 5
	@echo "\n==> Donate to project 1"
	$(MAKE) donate project_id=1 amount=10000 || exit 1

	@sleep 5
	@echo "\n==> Unlock project 1"
	$(MAKE) unlock project_id=1 || exit 1

	@sleep 5
	@echo "\n==> Disburse funds for project 1"
	$(MAKE) disburse project_id=1 || exit 1

	@sleep 5
	@echo "\n==> Query project 1"
	$(MAKE) get_project project_id=1 || exit 1

	@echo "\nFull flow test completed."


