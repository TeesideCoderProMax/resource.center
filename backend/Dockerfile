# syntax=docker/dockerfile:1
FROM rust:1.94.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --release --locked --bin loan-service --bin notifications-worker

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 appgroup \
    && useradd --system --uid 10001 --gid appgroup --no-create-home --shell /usr/sbin/nologin appuser

WORKDIR /app
COPY --from=builder /workspace/target/release/loan-service /usr/local/bin/loan-service
COPY --from=builder /workspace/target/release/notifications-worker /usr/local/bin/notifications-worker

USER 10001:10001
ENV PORT=8080
EXPOSE 8080

ENTRYPOINT ["loan-service"]
CMD ["api"]
