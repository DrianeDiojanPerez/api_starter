set dotenv-load := true
set shell := ["bash", "-euo", "pipefail", "-c"]

bruno_dir := "api/bruno/API"

# The DB_* variables come from .env and are expanded by the shell, not by just.
database_url := '"postgres://$DB_USERNAME:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_DATABASE?sslmode=disable"'
dbmate := "docker compose run --rm -T -e DATABASE_URL=" + database_url + " migrate"

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

# Run the test suite.
test:
    cargo test --all-targets

# Fail on any clippy warning.
lint:
    cargo clippy --all-targets -- -D warnings

# Format the workspace.
fmt:
    cargo fmt --all

# Verify formatting without rewriting anything.
fmt-check:
    cargo fmt --all --check

# Everything CI would run.
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

# ──── Migrations (dbmate) ──────────────────────────────────

# Apply every migration.
migrate: migrate-iam

# Apply the iam migrations.
migrate-iam:
    {{ dbmate }} --migrations-dir=/db/migrations/iam up

# Roll the last iam migration back.
migrate-iam-down:
    {{ dbmate }} --migrations-dir=/db/migrations/iam rollback

# Show which migrations have been applied.
migrate-status:
    {{ dbmate }} --migrations-dir=/db/migrations/iam status

# Drop the database. Destructive.
migrate-drop: terminate-connections
    {{ dbmate }} drop

# Scaffold a migration, for example: just migrate-new iam create_sessions_table
migrate-new module name:
    ./scripts/migrate/dbmate.sh {{ module }} {{ name }}

# Kick every open session off the database so it can be dropped.
[private]
terminate-connections:
    docker exec api-starter-db psql -U "$DB_USERNAME" -d postgres \
        -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '$DB_DATABASE' AND pid <> pg_backend_pid();" \
        >/dev/null 2>&1 || true

# ──── API collection (bruno) ──────────────────────────────────

# Run the whole Bruno collection against a running server.
api:
    cd {{ bruno_dir }} && npx --yes @usebruno/cli run . -r --env development

# Run one Bruno folder, for example: just api-folder auth
api-folder folder:
    cd {{ bruno_dir }} && npx --yes @usebruno/cli run {{ folder }} -r --env development

# ──── Local setup ──────────────────────────────────

# Copy .env, start the backing services and migrate.
setup:
    test -f .env || cp .env.example .env
    just services
    @echo "waiting for postgres"
    until docker exec api-starter-db pg_isready -U "$DB_USERNAME" -d "$DB_DATABASE" >/dev/null 2>&1; do sleep 1; done
    just migrate
    @echo "ready, run: just run"
