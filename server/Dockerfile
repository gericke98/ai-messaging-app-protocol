# Stage 1: Build
FROM rust:1.85-bookworm AS builder
WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build actual application
COPY src/ src/
COPY migrations/ migrations/
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ainm-server /usr/local/bin/ainm-server
COPY --from=builder /app/migrations /migrations
COPY config/ /config/

EXPOSE 8080 8443
HEALTHCHECK --interval=30s --timeout=3s CMD curl -sf http://localhost:8080/health/live || exit 1
ENTRYPOINT ["ainm-server"]
