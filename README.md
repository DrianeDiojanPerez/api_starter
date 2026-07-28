# API Starter (Rust)

Rust port of the API Starter backend, scoped to the **auth**, **user** and
**permission** parts of the IAM module. Built with axum, sqlx and tracing,
keeping the hexagonal layout (`module/<name>/{core,adapter}`) of the original
Go service.

## Stack

| Concern         | Crate                                  |
| --------------- | -------------------------------------- |
| HTTP server     | `axum` + `tower-http`                  |
| Async runtime   | `tokio`                                |
| Logging         | `tracing` + `tracing-subscriber` (JSON) + `tracing-appender` |
| Database        | `sqlx` (PostgreSQL, runtime checked queries) |
| Validation      | `validator`                            |
| Tokens          | `jsonwebtoken` (HS256)                 |
| Password hashes | `bcrypt`                               |
| Mail            | `lettre`                               |
| Migrations      | `dbmate` (unchanged from the Go service) |

`sqlx` runs with runtime checked queries on purpose, so neither `cargo build`
nor the Docker image build needs a reachable database.

## Layout

```
src/
├── main.rs                  entrypoint
├── config/                  environment backed configuration
├── database/                connection pool and transaction manager
├── provider/                composition root, wires every dependency
├── sdk/                     types shared across modules
├── server/                  router, global middleware, error handling
│   └── middlewares/         authentication, authorization, request context
├── shared/                  auth, rbac, jwt, mail, errors, pagination
└── module/
    ├── auth/                login, refresh, forgot and reset password
    └── iam/                 users and permissions
        ├── core/            domain, ports, services
        └── adapter/         handlers and repositories
migration/iam/               dbmate migrations
```

## Endpoints

Public:

| Method | Path                  | Description                     |
| ------ | --------------------- | ------------------------------- |
| POST   | `/v1/login`           | Issue an access + refresh token |
| POST   | `/v1/refresh-token`   | Exchange a refresh token        |
| POST   | `/v1/forgot-password` | Mail a password reset link      |
| POST   | `/v1/reset-password`  | Consume a reset token           |

Authenticated (`Authorization: Bearer <token>`):

| Method | Path                    | Permission        |
| ------ | ----------------------- | ----------------- |
| GET    | `/v1/users`             | `Users.View All`  |
| POST   | `/v1/users`             | authenticated     |
| GET    | `/v1/users/my-user`     | authenticated     |
| PATCH  | `/v1/users/my-user`     | authenticated     |
| GET    | `/v1/users/{user-id}`   | authenticated     |
| PATCH  | `/v1/users/{user-id}`   | authenticated     |
| DELETE | `/v1/users/{user-id}`   | authenticated     |
| GET    | `/v1/permissions`       | authenticated     |

Members of the `Admin` role bypass every permission check.

Every response uses one envelope:

```json
{ "data": { }, "pagination": { }, "error": null }
```

## Quick start (Docker)

```bash
cp .env.example .env
docker compose --profile dev up -d database mail
make migrate-all-up
docker compose --profile dev up --build dev
```

- API: http://localhost:3000
- Mailhog UI: http://localhost:8025

The seeded administrator is `admin@example.com`.

## Running locally

```bash
cp .env.example .env      # then set DB_HOST=127.0.0.1
docker compose --profile dev up -d database mail
make migrate-all-up
cargo run
```

## Development

```bash
make test     # cargo test --all-targets
make lint     # cargo clippy --all-targets -- -D warnings
make fmt      # cargo fmt --all
make check    # fmt + lint + test
```

## Migrations

Migrations stay in dbmate format, one directory per module:

```bash
make migrate-iam-up
make migrate-iam-down
./scripts/migrate/dbmate.sh iam create_something_table   # new migration
```

## Docker targets

The `Dockerfile` is multi stage, matching the Go setup:

- `development` hot reloads through `cargo watch`
- `test` runs `cargo test --all-targets`
- `builder` produces the release binary
- `production` ships that binary on `debian:bookworm-slim` as a non root user

Compose profiles: `dev`, `uat`, `prod`, `test`, `migrate`.

## Logging

Structured JSON is written to a daily rotated file under `LOGGER_DIRECTORY`
(`storage/logs/log.<date>.log`) and, outside production, to stdout. Every log
line inside a request carries `route_path`, `request_id`, `ip_address` and
`method`. `RUST_LOG` overrides `LOGGER_LEVEL` when set.

## Notes on the port

- Password reset tokens are stored as a SHA-256 digest instead of in clear
  text, so the table cannot be replayed if it leaks. The token mailed to the
  user is unchanged.
- `PATCH /v1/users/...` takes a typed optional payload rather than an untyped
  map, so an unknown field is rejected instead of silently ignored.
- Password hashes remain bcrypt, which keeps hashes interchangeable with the
  Go service during a migration.
