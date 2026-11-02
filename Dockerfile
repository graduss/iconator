# syntax=docker/dockerfile:1

FROM rust:1.98-slim-trixie AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
# build.rs needs only the lookup table; the SVGs are mounted at runtime
COPY icons/lookup_table.rs ./icons/
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && cp target/release/iconator /usr/local/bin/iconator

FROM debian:trixie-slim
# curl is used by the compose healthcheck
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/iconator /usr/local/bin/iconator

USER nobody
# pass the SVG directory in ICONS_DIR and mount it (see compose.yml)
EXPOSE 3000

CMD ["iconator"]
