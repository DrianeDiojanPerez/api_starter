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
| Migrations      | `sqlx::migrate!`, embedded in the binary |

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
migrations/                  sqlx migrations, embedded at compile time
api/bruno/API/               Bruno request collection
justfile                     task runner recipes
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
just up
```

The server applies the migrations itself on start up, so there is no separate
migrate step.

- API: http://localhost:3000
- Mailhog UI: http://localhost:8025

The seeded administrator is `admin@example.com`.

## Running locally

```bash
just setup    # copies .env and starts postgres and mailhog
just run
```

`.env.example` points at `127.0.0.1`, so the host tooling works out of the
box. Compose overrides `DB_HOST` and `MAIL_HOST` for the containerised app.

## Development

`just` is the task runner. Run `just` on its own for the full list.

```bash
just setup         # copy .env, start postgres and mailhog
just run           # cargo run
just watch         # cargo watch -x run
just test          # unit and HTTP tests, no infrastructure needed
just test-all      # the above plus the tests that need a database
just lint          # cargo clippy --all-targets -- -D warnings
just fmt           # cargo fmt --all
just check         # fmt-check + lint + test
just up / down / logs / ps
just api           # run the Bruno collection against a running server
```

A `Makefile` with the same targets is kept for parity with the Go repository.

## Tests

Three layers, all runnable with one command each:

| Layer                                | Where                             | Needs a database |
| ------------------------------------ | --------------------------------- | ---------------- |
| Unit tests, fakes for the boundaries  | `#[cfg(test)]` next to the code    | no               |
| HTTP tests through the real router    | `tests/auth_routes.rs`, `tests/iam_routes.rs` | no   |
| Repository, transaction and RBAC tests | `tests/postgres.rs`               | yes              |

`tests/support/mod.rs` holds the fakes that stand in for the auth, RBAC, user
and permission services, so the HTTP tests exercise the real middleware stack,
extractors, routing and error rendering without any infrastructure.

`tests/postgres.rs` **skips itself** unless `TEST_DATABASE_URL` is set, which
keeps `cargo test` green on a bare checkout. An empty database is enough,
since the tests apply the embedded migrations themselves:

```bash
just test        # skips the database layer
just test-all    # starts postgres and runs everything
```

Those tests namespace every row they insert, so they are safe to run in
parallel and against a database that already holds the seed data.

## Migrations

Migrations live in `migrations/` as reversible sqlx pairs
(`<version>_<name>.up.sql` and `.down.sql`) and are **compiled into the
binary** by `sqlx::migrate!`. The production image therefore ships without any
SQL files, and every environment runs byte for byte the same schema.

The server applies whatever is pending when it starts. sqlx takes an advisory
lock first, so several replicas booting together is safe. Set
`DB_RUN_MIGRATIONS=false` to gate schema changes behind a separate step
instead.

To drive them by hand, install `sqlx-cli` once:

```bash
just migrate-install                    # cargo install sqlx-cli
just migrate                            # apply what is pending
just migrate-down                       # revert the last one
just migrate-status                     # what has been applied
just migrate-new create_foo_table       # scaffold a reversible pair
just migrate-reset                      # drop, recreate, migrate (destructive)
```

## API collection

`api/bruno/API` is a [Bruno](https://usebruno.com) collection covering every
endpoint, laid out as `auth/` and `iam/{user,permission}/`.

Open the folder in Bruno and pick the `development` environment, or run it
headless:

```bash
just api                # whole collection
just api-folder auth    # one folder
```

`Login` stores `token`, `refresh_token` and `user_id` on the environment, and
every other request reads them from there, so run it first.

`Reset Password` is the one manual request: the reset token only exists in the
mail `Forgot Password` sends, since the database holds a digest of it. Grab it
from Mailhog on http://localhost:8025 and put it in the `reset_token`
environment variable.

## Docker targets

The `Dockerfile` is multi stage, matching the Go setup:

- `development` hot reloads through `cargo watch`
- `test` runs `cargo test --all-targets`
- `builder` produces the release binary
- `production` ships that binary on `debian:bookworm-slim` as a non root user

Compose profiles: `dev`, `uat`, `prod`, `test`.

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
