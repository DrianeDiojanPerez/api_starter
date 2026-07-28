include .env
export

.PHONY: run build test lint fmt check up down logs migrate migrate-down migrate-status

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
up:
	@docker compose --profile dev up --build
down:
	@docker compose --profile dev down
logs:
	@docker compose --profile dev logs -f dev

# ──── Migrations (sqlx) ──────────────────────────────────
# The server applies these on start up. Use these targets to drive them by
# hand: cargo install sqlx-cli --version '~0.8' --no-default-features --features rustls,postgres
migrate:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate run
migrate-down:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate revert
migrate-status:
	@DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate info
