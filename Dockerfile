# ──── Base ──────────────────────────────────
FROM rust:1.93-bookworm AS base

WORKDIR /app

ENV CARGO_TERM_COLOR=always

# Dependency layer: only the manifests, so a source change does not rebuild
# the whole dependency tree.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# ──── Dev ──────────────────────────────────
FROM base AS development

ARG USERNAME=appuser

RUN groupadd --force -g 1000 $USERNAME \
    && useradd -ms /bin/bash --no-user-group -g 1000 -u 1000 $USERNAME \
    && apt-get update \
    && apt-get install -y --no-install-recommends tzdata \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-watch --locked

COPY . .

RUN chown -R $USERNAME:$USERNAME /app

ENV TZ="America/Belize"

USER $USERNAME

CMD ["cargo", "watch", "-x", "run"]

# ──── Test ──────────────────────────────────
FROM base AS test

COPY . .

CMD ["cargo", "test", "--all-targets"]

# ──── Builder ──────────────────────────────────
FROM base AS builder

COPY . .

RUN touch src/main.rs && cargo build --release --locked

# ──── Prod ──────────────────────────────────
FROM debian:bookworm-slim AS production

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g 1000 appuser \
    && useradd -m -u 1000 -g 1000 appuser

ENV TZ="America/Belize"

WORKDIR /app

COPY --from=builder /app/target/release/server /usr/local/bin/server

RUN mkdir -p /app/storage/logs && chown -R appuser:appuser /app

USER appuser

EXPOSE 3000

CMD ["server"]
