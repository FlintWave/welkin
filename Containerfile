# syntax=docker/dockerfile:1
# Multi-stage build for welkin binaries.
# Produces static musl-linked binaries in a minimal runtime image.
#
# Build:  podman build -t welkin .
# Run:    podman run --rm welkin robust
#         podman run --rm welkin region

# ── Stage 1: chef (dependency cache layer) ──────────────────────────
FROM docker.io/library/rust:1.87-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /src

# ── Stage 2: planner (compute recipe from lock file) ────────────────
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: builder (compile with musl + C toolchain) ──────────────
FROM chef AS builder

# musl cross-compilation target
RUN rustup target add x86_64-unknown-linux-musl

# C toolchain for native deps (openjpeg-sys via aurora-assets).
# cmake + g++ added now so the stage extends for SLua (C++) later.
RUN apt-get update && apt-get install -y --no-install-recommends \
        musl-tools \
        cmake \
        g++ \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

ENV CC_x86_64_unknown_linux_musl=musl-gcc
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc

# Cache dependencies (rebuilt only when Cargo.toml/lock change)
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Build the actual binaries
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --target x86_64-unknown-linux-musl \
        --bin welkin-robust \
        --bin welkin-region \
    && strip target/x86_64-unknown-linux-musl/release/welkin-robust \
    && strip target/x86_64-unknown-linux-musl/release/welkin-region

# ── Stage 4: runtime (minimal image, non-root) ─────────────────────
FROM docker.io/library/alpine:3.21 AS runtime

RUN addgroup -S opensim && adduser -S opensim -G opensim

COPY --from=builder --chown=opensim:opensim \
    /src/target/x86_64-unknown-linux-musl/release/welkin-robust \
    /usr/local/bin/welkin-robust
COPY --from=builder --chown=opensim:opensim \
    /src/target/x86_64-unknown-linux-musl/release/welkin-region \
    /usr/local/bin/welkin-region

USER opensim
WORKDIR /home/opensim

# Health check on the metrics/health endpoint (default port 9000)
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:9000/health || exit 1

# LLUDP (viewer traffic) — UDP must be published explicitly with -p
EXPOSE 9000/tcp
EXPOSE 9000/udp

ENTRYPOINT []
CMD ["welkin-robust"]
