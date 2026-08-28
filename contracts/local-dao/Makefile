MAKEFLAGS += --no-print-directory

build:
	RUSTFLAGS="-C target-cpu=mvp -C link-arg=--allow-undefined" cargo +nightly build -Zbuild-std=panic_abort,std \
		--target wasm32-unknown-unknown --release --lib

schema:
	cargo run --bin schema

test:
	cargo test

KEY_NAME=greg
CHAIN_ID=testing
NODE=http://localhost:26657
DENOM=ustake
WASM_PATH=../../target/wasm32-unknown-unknown/release/local_dao.wasm

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
	INIT_MSG='{"quorum_bps":3000,"voting_period_seconds":604800}'; \
	echo "Using CODE_ID=$$CODE_ID, ADMIN=$$ADDR, INIT_MSG=$$INIT_MSG"; \
	TX_HASH=$$( \
	wasmd tx wasm instantiate $$CODE_ID "$$INIT_MSG" \
		--from $(KEY_NAME) \
		--label "local-dao" \
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
