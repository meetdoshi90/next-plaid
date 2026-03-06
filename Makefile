.PHONY: all build release test lint fmt fmt-check check clean \
        docker-build docker-up docker-down docker-logs bump-version

all: fmt lint test

build:
	cargo build

release:
	cargo build --release -p next-plaid-api

test:
	cargo test

lint: fmt-check clippy

clippy:
	cargo clippy --all-targets -- -D warnings

fmt-check:
	cargo fmt --all -- --check

fmt:
	cargo fmt --all

check:
	cargo check

clean:
	cargo clean

docker-build:
	docker build -t next-plaid-api -f next-plaid-api/Dockerfile .

docker-up:
	docker compose up -d

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f

launch-debug:
	-kill -9 $$(lsof -t -i:8080) 2>/dev/null || true
	rm -rf next-plaid-api/indices
	cd next-plaid-api && RUST_LOG=debug cargo run --release

# Usage: make bump-version VERSION=1.1.0
bump-version:
ifndef VERSION
	$(error VERSION is required. Usage: make bump-version VERSION=1.1.0)
endif
	@sed -i '' '/^\[workspace\.package\]/,/^\[/{s/^version = "[^"]*"/version = "$(VERSION)"/;}' Cargo.toml
	@sed -i '' 's/next-plaid:cpu-[0-9]*\.[0-9]*\.[0-9]*/next-plaid:cpu-$(VERSION)/g' next-plaid-api/README.md
	@sed -i '' '/^\[project\]/,/^\[/{s/^version = "[^"]*"/version = "$(VERSION)"/;}' next-plaid-api/python-sdk/pyproject.toml
	@sed -i '' 's/__version__ = "[^"]*"/__version__ = "$(VERSION)"/' next-plaid-api/python-sdk/next_plaid_client/__init__.py
	@cargo check --quiet
	@echo "Version bumped to $(VERSION)"