# ── Build stage ──
# Note: rustc ≥ 1.89 required by redb 4.1.0 + darling 0.23.0.
# Toolchain pin tracks Cargo.lock — bump when Cargo.toml dependencies move.
FROM rust:1.89-slim-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release && \
    strip target/release/taodb

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/taodb /usr/local/bin/taodb

RUN useradd --system --create-home taodb
USER taodb

VOLUME ["/data"]
EXPOSE 8765

ENTRYPOINT ["taodb"]
CMD ["serve", "--addr", "0.0.0.0:8765", "--data", "/data"]
