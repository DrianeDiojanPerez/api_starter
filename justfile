set dotenv-load := true
set shell := ["bash", "-euo", "pipefail", "-c"]

bruno_dir := "api/bruno/API"

# The DB_* variables come from .env and are expanded by the shell, not by just.
database_url := '"postgres://$DB_USERNAME:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_DATABASE"'
sqlx := "DATABASE_URL=" + database_url + " cargo sqlx"

# List every recipe.
default:
    @just --list --unsorted

# ──── Application ──────────────────────────────────

# Run the server.
run:
    cargo run

# Run the server, restarting on any source change.
watch:
    cargo watch -x run

# Build the release binary.
build:
    cargo build --release --locked

# Run the unit and HTTP tests. No database needed.
test:
    cargo test --all-targets

# Run everything, including the tests that need a database.
test-all: services
    TEST_DATABASE_URL={{ database_url }} cargo test --all-targets

# Fail on any clippy warning.
lint:
    cargo clippy --all-targets -- -D warnings

# Format the workspace.
fmt:
    cargo fmt --all

# Verify formatting without rewriting anything.
fmt-check:
    cargo fmt --all --check

# Everything CI would run without infrastructure.
check: fmt-check lint test

# Remove build artifacts and local logs.
clean:
    cargo clean
    rm -rf storage/logs

# ──── Containers ──────────────────────────────────

# Create the shared network, which compose expects to already exist.
network:
    @docker network inspect api-starter-bridge >/dev/null 2>&1 \
        || docker network create api-starter-bridge

# Start the API, database and mail catcher.
up: network
    docker compose --profile dev up --build

# Start the stack without rebuilding, for when nothing in the image changed.
start: network
    docker compose --profile dev up -d --no-build

# Rebuild the image from scratch, ignoring every cached layer.
rebuild: network
    docker compose --profile dev build --no-cache dev
    docker compose --profile dev up

# Start only the backing services, for running the API on the host.
services: network
    docker compose --profile dev up -d database mail
    @echo "waiting for postgres"
    until docker exec api-starter-db pg_isready -U "$DB_USERNAME" -d "$DB_DATABASE" >/dev/null 2>&1; do sleep 1; done

# Stop the dev stack.
down:
    docker compose --profile dev down

# Stop the dev stack and drop its volumes, including the cargo caches.
down-hard:
    docker compose --profile dev down -v

# Follow the API logs.
logs:
    docker compose --profile dev logs -f dev

# Show the running containers.
ps:
    docker compose --profile dev ps

# Run the test suite inside the test image.
docker-test:
    docker compose --profile test run --rm test

# Build the production image.
docker-build: network
    docker compose --profile prod build prod

# ──── Migrations (sqlx) ──────────────────────────────────
#
# One directory per module under migrations/, so a module can be migrated on
# its own. The server applies all of them on start up unless
# DB_RUN_MIGRATIONS=false; these recipes drive them by hand and need sqlx-cli:
# just migrate-install

# Install sqlx-cli, needed only by the recipes below.
migrate-install:
    cargo install sqlx-cli --version '~0.8' --no-default-features --features rustls,postgres --locked

# Apply every module's pending migrations, in registry order.
migrate:
    cargo run --quiet -- migrate

# Apply one module, for example: just migrate-module iam
migrate-module module:
    cargo run --quiet -- migrate {{ module }}

# Revert the last migration of one module, for example: just migrate-down iam
migrate-down module:
    {{ sqlx }} migrate revert --source migrations/{{ module }} --ignore-missing

# Show what has been applied, for one module or all of them.
migrate-status module="":
    if [ -n "{{ module }}" ]; then \
        {{ sqlx }} migrate info --source migrations/{{ module }}; \
    else \
        for dir in migrations/*/; do \
            echo "==> $(basename "$dir")"; \
            {{ sqlx }} migrate info --source "$dir"; \
        done; \
    fi

# Scaffold a reversible pair, for example: just migrate-new catalog create_catalogs_table
# A brand new module also needs a line in src/database/migrations.rs.
migrate-new module name:
    mkdir -p migrations/{{ module }}
    {{ sqlx }} migrate add -r --source migrations/{{ module }} {{ name }}

# Drop and recreate the database, then migrate every module. Destructive.
migrate-reset:
    {{ sqlx }} database drop -y
    {{ sqlx }} database create
    just migrate

# ──── API collection (bruno) ──────────────────────────────────

# Run the whole Bruno collection against a running server.
api:
    cd {{ bruno_dir }} && npx --yes @usebruno/cli run . -r --env development

# Run one Bruno folder, for example: just api-folder auth
api-folder folder:
    cd {{ bruno_dir }} && npx --yes @usebruno/cli run {{ folder }} -r --env development

# ──── Local setup ──────────────────────────────────

# Copy .env and start the backing services. The server migrates itself.
setup:
    test -f .env || cp .env.example .env
    just services
    @echo "ready, run: just run"
