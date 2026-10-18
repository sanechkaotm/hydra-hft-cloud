# =============================================================================
# HYDRA-HFT v4.0 — MULTI-STAGE ULTRA-SLIM PRODUCTION CONTAINER
# =============================================================================
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /build
COPY Cargo.toml ./
COPY crates ./crates

ENV RUSTFLAGS="-C target-cpu=native"
RUN cargo build --release --bin hydra-node

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/hydra-node /usr/local/bin/hydra-node
COPY .env.example /app/.env.example

ENTRYPOINT ["/usr/local/bin/hydra-node"]
CMD ["--role", "tokyo", "--dry-run"]
