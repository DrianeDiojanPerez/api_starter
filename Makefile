include .env
export

.PHONY: run build test lint fmt check network up down logs migrate migrate-module migrate-down migrate-status

DATABASE_URL := postgres://$(DB_USERNAME):$(DB_PASSWORD)@$(DB_HOST):$(DB_PORT)/$(DB_DATABASE)

# ──── Application ──────────────────────────────────
run:
	@cargo run
build:
	@cargo build --release --locked
test:
	@cargo test --all-targets
lint:
	@cargo clippy --all-targets -- -D warnings
fmt:
	@cargo fmt --all
check: fmt lint test

# ──── Containers ──────────────────────────────────
network:
	@docker network inspect api-starter-bridge >/dev/null 2>&1 \
		|| docker network create api-starter-bridge
up: network
	@docker compose --profile dev up --build
down:
	@docker compose --profile dev down
logs:
	@docker compose --profile dev logs -f dev

# ──── Migrations (sqlx) ──────────────────────────────────
# One directory per module under migrations/. The server applies all of them on
# start up. Use these targets to drive them by hand:
# cargo install sqlx-cli --version '~0.8' --no-default-features --features rustls,postgres
#
# Pass MODULE to target one: make migrate-module MODULE=iam
MODULE ?= iam

migrate:
	@for dir in migrations/*/; do \
		echo "==> $$(basename $$dir)"; \
		DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate run --source $$dir --ignore-missing; \
	done
migrate-module:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate run --source migrations/$(MODULE) --ignore-missing
migrate-down:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate revert --source migrations/$(MODULE) --ignore-missing
migrate-status:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate info --source migrations/$(MODULE)
