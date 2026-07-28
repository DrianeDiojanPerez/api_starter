include .env

DATABASE_URL := postgres://$(DB_USERNAME):$(DB_PASSWORD)@$(DB_HOST):$(DB_PORT)/$(DB_DATABASE)?sslmode=disable
DBMATE := docker compose run --rm -it -e DATABASE_URL=$(DATABASE_URL) migrate

.PHONY: run build test lint fmt check up down logs migrate-all-up migrate-all-down migrate-iam-up migrate-iam-down terminate-db-connections

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

# ──── Migrations (dbmate) ──────────────────────────────────
migrate-all-up: migrate-iam-up

migrate-all-down: terminate-db-connections
	@$(DBMATE) drop
terminate-db-connections:
	@docker exec api-starter-db psql -U $(DB_USERNAME) -d postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '$(DB_DATABASE)' AND pid <> pg_backend_pid();" >/dev/null 2>&1 || true
migrate-iam-up:
	@$(DBMATE) --migrations-dir=/db/migrations/iam up
migrate-iam-down:
	@$(DBMATE) --migrations-dir=/db/migrations/iam rollback
