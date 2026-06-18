# syntax=docker/dockerfile:1
FROM rust:1-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev perl make && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY xtask ./xtask
COPY agents ./agents
COPY packages ./packages
# Optional build args for dev environments to speed up compilation
# Example: docker build --build-arg LTO=false --build-arg CODEGEN_UNITS=16 .
ARG LTO=true
ARG CODEGEN_UNITS=1
ENV CARGO_PROFILE_RELEASE_LTO=${LTO} \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=${CODEGEN_UNITS}
RUN cargo build --release --bin openfang

FROM rust:1-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    python3 \
    python3-pip \
    python3-venv \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/openfang /usr/local/bin/
# Baked agents seeded to the data volume on first boot (Dewansh + assistant).
# Curated deploy set — NOT the repo's example agent gallery under ./agents.
COPY deploy/agents /opt/openfang/agents
# Baked default config (seeded to the data volume on first boot if absent).
COPY deploy/config.default.toml /opt/openfang/config.toml
EXPOSE 4200
VOLUME /data
ENV OPENFANG_HOME=/data
# On boot: seed baked agents into the PVC, and seed the default config if the
# volume has none yet. Existing files on the volume are never overwritten.
ENTRYPOINT ["/bin/sh","-c","mkdir -p /data/agents && cp -rn /opt/openfang/agents/. /data/agents/ 2>/dev/null || true; [ -f /data/config.toml ] || cp /opt/openfang/config.toml /data/config.toml 2>/dev/null || true; exec openfang \"$@\"","--"]
CMD ["start"]
