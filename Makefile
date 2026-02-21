.PHONY: all build build-debug test test-unit test-bridge lint lint-fmt lint-clippy bench clean install

all: build

build:
	cargo build --workspace --release

build-debug:
	cargo build --workspace

test: test-unit test-bridge

test-unit:
	cargo test --lib

test-bridge:
	cargo test --test "bridge*"

lint: lint-fmt lint-clippy

lint-fmt:
	cargo fmt --all -- --check

lint-clippy:
	cargo clippy --workspace --all-targets -- -D warnings

bench:
	cargo bench

clean:
	cargo clean

install:
	cargo install --path crates/agentic-memory-mcp
