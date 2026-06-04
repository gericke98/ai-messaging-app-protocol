# Stage 1: build the ainm-server workspace member.
FROM rust:1.85-bookworm AS builder
WORKDIR /build

# Copy the full workspace (the .dockerignore trims target/, ios/, etc.).
# A workspace member depends on ../crates/*, so the whole workspace must be present —
# this is the fix for the old server/-context Dockerfile that could not resolve it.
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
COPY crates/ crates/

RUN cargo build --release -p ainm-server

# Stage 2: minimal runtime.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl bubblewrap \
 && rm -rf /var/lib/apt/lists/*
# Non-root runtime user.
RUN useradd --system --uid 10001 --create-home ainm
WORKDIR /app
COPY --from=builder /build/target/release/ainm-server /usr/local/bin/ainm-server
COPY --from=builder /build/server/migrations /app/migrations
COPY --from=builder /build/server/config /app/config
USER ainm
EXPOSE 8080 8443
# /health/ready is DB-aware (deeper than the always-200 /health/live).
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s \
  CMD curl -sf http://localhost:8080/health/ready || exit 1
ENTRYPOINT ["ainm-server"]
