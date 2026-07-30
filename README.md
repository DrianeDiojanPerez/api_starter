# API Starter

Starting point for a Rust API service: **authentication**, **users** and
**permissions**, ready to build the rest of a product on top of. Built with
axum, sqlx and tracing.

Laid out hexagonally (`module/<name>/{core,adapter}`) so new modules drop in
beside `auth` and `iam` without touching anything else.

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
├── package/                 everything the modules share, see below
├── provider/                composition root, wires every dependency
├── server/                  router, global middleware, error handling
│   └── middlewares/         authentication, authorization, request context
└── module/
    ├── auth/                login, refresh, forgot and reset password
    └── iam/                 users and permissions
        ├── core/            domain, ports, services
        └── adapter/         handlers and repositories
migrations/<module>/         sqlx migrations, one directory per module
api/bruno/API/               Bruno request collection
justfile                     task runner recipes
```

### The `package` folder

Everything the modules build on lives in `package`, so there is one answer to
"where does this go": if it is not a module, it is a package.

| Module       | What it holds                                          |
| ------------ | ------------------------------------------------------ |
| `auth`       | login, refresh, password recovery, the `Identity` type |
| `crypto`     | password hashing, random tokens, token digests         |
| `emailer`    | SMTP transport and the embedded templates              |
| `env`        | typed environment reads with fallbacks                 |
| `errdef`     | the error type and how it renders as HTTP              |
| `extract`    | JSON and validating extractors                         |
| `jwt`        | HS256 signing and validation                           |
| `logger`     | tracing subscriber, JSON to a daily rotated file       |
| `masked`     | secret wrappers that stay redacted in logs and JSON    |
| `pagination` | list requests, filters, sorting and the page envelope  |
| `rbac`       | permission checks, `Resource.Permission` actions       |
| `response`   | the single response envelope                           |
| `validation` | password rules and flattened validator output          |

The lower half of that list is liftable into another service more or less
unchanged; the upper half encodes decisions specific to this API. Nothing
enforces the difference, so if you copy `env` or `crypto` out, check what
comes with it.

`config` is the only caller of `env`, so the rules about blank values,
trimming and fallbacks are written once:

```rust
env::string_or("APP_NAME", "App_sample")     // unset or blank falls back
env::required("JWT_SECRET")?                 // refuses to start without it
env::u16_or("APP_PORT", 3000)?               // rejects 70000, it is a port
env::u32_or("DB_MAX_CONNECTIONS", 10)?       // rejects -1, a pool is unsigned
env::i64_or("ACCESS_TOKEN_TTL", 3600)?       // seconds
env::boolean_or("DB_RUN_MIGRATIONS", true)?  // 1/yes/on as well as true
env::vec_or("CORS_ORIGINS", &["*"])          // comma separated, trimmed
env::variant_or_default("LOGGER_LEVEL")?     // one of a fixed set of names
```

Every reader is named for the type it hands back, so the width is checked at
the edge rather than cast into shape later. The parse itself is private: there
is no generic `parsed::<T>` to reach for, and that is what keeps the call sites
reading as concrete types.

`variant_or_default` is the one generic reader, because `env` cannot name the
enums that live in `config`.

A value that is present but unparseable is an error, not a silent fallback: a
typo in a deployment should stop the process at start up rather than quietly
running with a default nobody asked for.

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
just up            # build the image and start the stack
just start         # start it again without rebuilding
just rebuild       # rebuild from scratch, ignoring cached layers
just down          # stop it
just down-hard     # stop it and drop the volumes, cargo caches included
just logs / ps
just api           # run the Bruno collection against a running server
```

A `Makefile` mirroring the same targets is included as well.

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

Migrations live in `migrations/<module>/` as reversible sqlx pairs
(`<version>_<name>.up.sql` and `.down.sql`), **one directory per module**, so a
module can be migrated on its own. They are compiled into the binary by
`sqlx::migrate!`, so the production image ships without any SQL files and every
environment runs byte for byte the same schema.

```
migrations/
└── iam/
    ├── 20250609223816_create_schema.up.sql
    ├── 20250609223816_create_schema.down.sql
    └── ...
```

`src/database/migrations.rs` is the registry, and its order is the order they
are applied. That matters once one module references another's tables: put a
module after the ones it depends on.

```rust
pub fn all() -> Vec<ModuleMigrations> {
    vec![
        module("iam", sqlx::migrate!("./migrations/iam")),
        // module("catalog", sqlx::migrate!("./migrations/catalog")),
    ]
}
```

The server applies everything pending when it starts. sqlx takes an advisory
lock first, so several replicas booting together is safe. Set
`DB_RUN_MIGRATIONS=false` to gate schema changes behind a separate step, and
run them with the binary itself:

```bash
server migrate           # every module, in registry order
server migrate iam       # one module
```

All modules share the single `_sqlx_migrations` ledger. Each module's migrator
ignores the versions the others recorded, and versions are timestamps, so they
never collide.

### Adding a module

```bash
just migrate-new catalog create_catalogs_table   # creates migrations/catalog/
```

Then add one line to `all()` in `src/database/migrations.rs`. The macro reads
the directory at compile time, so the binary always carries what the source
tree holds.

### Recipes

`migrate` and `migrate-module` go through the binary, so they respect the
registry order. The rest need `sqlx-cli`, installed once with
`just migrate-install`.

```bash
just migrate                          # every module, registry order
just migrate-module iam               # one module
just migrate-down iam                 # revert that module's last migration
just migrate-status                   # per module, what has been applied
just migrate-status iam               # one module
just migrate-new iam create_foo_table # scaffold a reversible pair
just migrate-reset                    # drop, recreate, migrate (destructive)
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

The `Dockerfile` is multi stage:

- `development` hot reloads through `cargo watch`
- `test` runs `cargo test --all-targets`
- `builder` produces the release binary
- `production` ships that binary on `debian:bookworm-slim` as a non root user

Compose profiles: `dev`, `uat`, `prod`, `test`.

## Logging

Structured JSON is written to a daily rotated file under `LOGGER_DIRECTORY`
(`storage/logs/log.<date>.log`) and, outside production, to stdout. Every log
line inside a request carries `route_path`, `request_id`, `ip_address` and
`method`.

`LOGGER_LEVEL` takes one of `trace|debug|info|warn|error` and defaults to
`info`. `RUST_LOG` overrides it when set, and accepts the full tracing filter
syntax.

`APP_ENVIRONMENT` takes one of `local|development|production` and defaults to
`development`; only `production` turns the stdout logger off. Both variables
reject anything else at start up rather than falling back to a default.

## Design notes

- Password reset tokens are stored as a SHA-256 digest, never in clear text,
  so the table cannot be replayed if it leaks. Only the mailed token is usable.
- `PATCH /v1/users/...` takes a typed optional payload rather than an untyped
  map, so an unknown field is rejected instead of silently ignored.
