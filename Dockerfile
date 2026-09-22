# syntax=docker/dockerfile:1.7

FROM rust:1.95-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps ./apps
COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release -p aro-api && \
    cp /app/target/release/aro-api /tmp/aro-api

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251 AS runtime
LABEL org.opencontainers.image.title="ARO API" \
      org.opencontainers.image.description="ARO cloud sync API" \
      org.opencontainers.image.source="https://github.com/Hakille-ai/Aro" \
      org.opencontainers.image.vendor="ARO"

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    useradd --create-home --system --uid 10001 --shell /usr/sbin/nologin aro

COPY --from=builder /tmp/aro-api /usr/local/bin/aro-api

USER aro
EXPOSE 8710
ENV ARO_API_BIND=0.0.0.0:8710 \
    ARO_ENV=production \
    ARO_DEPLOYMENT_MODE=self-host \
    ARO_LOG_FORMAT=json

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8710/live || exit 1

ENTRYPOINT ["aro-api"]
CMD ["serve"]
