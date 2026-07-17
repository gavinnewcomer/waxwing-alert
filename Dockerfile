# syntax=docker/dockerfile:1

# ---- build stage ----
# Bookworm-based so its glibc matches the distroless/cc-debian12 runtime below.
FROM rust:1-slim-bookworm AS builder
# `ring` (via rustls) needs a C compiler + perl to build.
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential perl pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release --bin ebird-alert

# ---- runtime stage ----
# Tiny, no shell/package manager; includes glibc, libgcc, and ca-certificates for TLS.
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/ebird-alert /usr/local/bin/ebird-alert
# The bot writes ebird-alert-state.json to its working directory — mount a volume at /data
# to persist keys + subscriptions across restarts.
WORKDIR /data
ENTRYPOINT ["/usr/local/bin/ebird-alert"]
