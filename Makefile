# Aquachain contracts local gates

CLIPPY_FLAGS := -D warnings
CARGO ?= cargo

.PHONY: help fmt format lint test coverage coverage-summary coverage-html tarpaulin \
	machete outdated fuzz fuzz-build geiger audit deny doc ci

.DEFAULT_GOAL := help

help:
	@echo "Aquachain contracts targets"
	@echo "  make lint / test / doc / coverage / audit / deny / ci"

fmt:
	$(CARGO) fmt --all -- --check

format:
	$(CARGO) fmt --all

lint: fmt
	$(CARGO) clippy --workspace --all-targets -- $(CLIPPY_FLAGS)

test:
	$(CARGO) test --workspace

doc:
	RUSTDOCFLAGS='-D warnings' $(CARGO) doc --workspace --no-deps

coverage:
	mkdir -p coverage
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --workspace --locked --lcov --output-path coverage/lcov.info

coverage-summary:
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --workspace --locked --summary-only

coverage-html:
	mkdir -p coverage
	RUSTUP_TOOLCHAIN=stable $(CARGO) llvm-cov --workspace --locked --html --output-dir coverage/html

tarpaulin:
	mkdir -p coverage/tarpaulin
	$(CARGO) tarpaulin --workspace --locked --out Html --out Xml --output-dir coverage/tarpaulin

machete:
	$(CARGO) machete

outdated:
	$(CARGO) outdated --workspace

FUZZ_TARGET ?=
FUZZ_TIME ?= 10

fuzz-build:
	@test -d fuzz || (echo "no fuzz/; skip or add a cargo-fuzz workspace"; exit 1)
	cargo +nightly fuzz build

fuzz:
	@test -n "$(FUZZ_TARGET)" || (echo "set FUZZ_TARGET=…"; exit 1)
	cargo +nightly fuzz run $(FUZZ_TARGET) -- -max_total_time=$(FUZZ_TIME)

geiger:
	$(CARGO) geiger --workspace || true

audit:
	$(CARGO) audit

deny:
	$(CARGO) deny check

ci: lint test doc audit deny
