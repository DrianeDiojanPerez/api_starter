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

# Start the API, database and mail catcher.
up:
    docker compose --profile dev up --build -d

# Start only the backing services, for running the API on the host.
services:
    docker compose --profile dev up -d database mail
    @echo "waiting for postgres"
    until docker exec api-starter-db pg_isready -U "$DB_USERNAME" -d "$DB_DATABASE" >/dev/null 2>&1; do sleep 1; done

# Stop the dev stack.
down:
    docker compose --profile dev down

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
docker-build:
    docker compose --profile prod build prod

# ──── Migrations (sqlx) ──────────────────────────────────
#
# Migrations are embedded in the binary and applied on start up unless
# DB_RUN_MIGRATIONS=false. These recipes are for driving them by hand and
# need sqlx-cli: just migrate-install

# Install sqlx-cli, needed only by the recipes below.
migrate-install:
    cargo install sqlx-cli --version '~0.8' --no-default-features --features rustls,postgres --locked

# Apply every pending migration.
migrate:
    {{ sqlx }} migrate run

# Revert the last applied migration.
migrate-down:
    {{ sqlx }} migrate revert

# Show which migrations have been applied.
migrate-status:
    {{ sqlx }} migrate info

# Scaffold a reversible migration, for example: just migrate-new create_sessions_table
migrate-new name:
    {{ sqlx }} migrate add -r {{ name }}

# Drop and recreate the database, then migrate. Destructive.
migrate-reset:
    {{ sqlx }} database reset -y

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
