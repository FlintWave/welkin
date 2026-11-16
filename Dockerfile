# syntax=docker/dockerfile:1
# ============================================================================
# OpenSim Rust — Production Container Image
# ============================================================================
#
# Multi-stage build producing a minimal, hardened container image for the
# OpenSim Rust grid stack.  Builds all production binaries:
#
#   - welkin-robust   Grid services server (login, asset, inventory, etc.)
#   - welkin-region   Region simulator (scene, physics, scripting)
#   - welkin-portal   Web administration portal
#   - osim-migrate     Database migration and configuration tool
#
# SPDX-License-Identifier: LGPL-3.0-or-later
#
# Usage:
#   docker build -t welkin:latest .
#   docker run --rm welkin:latest                    # → robust
#   docker run --rm welkin:latest welkin-region     # → region
#   docker run --rm welkin:latest welkin-portal     # → portal
#   docker run --rm welkin:latest osim-migrate       # → migration
#
# ============================================================================

# ---------------------------------------------------------------------------
# Build argument: image version label
# Override at build time: docker build --build-arg VERSION=1.0.0 .
# ---------------------------------------------------------------------------
ARG VERSION=0.1.0

# ============================================================================
# Stage 1: chef — install cargo-chef for dependency caching
# ============================================================================
#
# cargo-chef splits the build into a dependency-only step and a source step,
# so that changing application code does not invalidate the (expensive)
# dependency compilation layer.
#
FROM docker.io/library/rust:1.83-bookworm AS chef

RUN cargo install cargo-chef --locked

WORKDIR /src

# ============================================================================
# Stage 2: planner — compute dependency recipe from the workspace
# ============================================================================
#
# The planner reads Cargo.toml and Cargo.lock across the entire workspace
# and produces a recipe.json that captures every crate dependency.  Only
# changes to dependencies trigger a rebuild of this layer and the cook step.
#
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates crates

RUN cargo chef prepare --recipe-path recipe.json

# ============================================================================
# Stage 3: builder — compile all release binaries
# ============================================================================
#
# This stage installs the native C toolchain (needed for openjpeg-sys via
# aurora-assets and luau via mlua), cooks the dependency cache, then builds
# the actual application binaries.
#
# The default target (x86_64-unknown-linux-gnu / glibc) is used because the
# runtime image (distroless/cc-debian12) ships the C runtime library.
#
FROM chef AS builder

# ---------------------------------------------------------------------------
# Native build dependencies
# ---------------------------------------------------------------------------
#   musl-tools   — not used here (glibc target), but left commented for musl
#   cmake        — required by openjpeg-sys and luau (mlua) builds
#   g++          — C++ compiler for luau and other native deps
#   pkg-config   — locates system libraries during compilation
#   libssl-dev   — OpenSSL headers for TLS support in reqwest/rustls fallback
# ---------------------------------------------------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake \
        g++ \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Dependency cache layer (cargo-chef cook)
# ---------------------------------------------------------------------------
# This layer is rebuilt ONLY when Cargo.toml or Cargo.lock change.
# All 24+ workspace crate dependencies are compiled here, which takes the
# bulk of the build time (~15–25 minutes on a cold cache).
# ---------------------------------------------------------------------------
COPY --from=planner /src/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

# ---------------------------------------------------------------------------
# Application source and release build
# ---------------------------------------------------------------------------
COPY Cargo.toml Cargo.lock ./
COPY crates crates

# Build all four production binaries in a single cargo invocation to share
# compilation artifacts across crates (they share a large common dependency
# tree: tokio, axum, sqlx, serde, etc.).
RUN cargo build --release \
        --bin welkin-robust \
        --bin welkin-region \
        --bin welkin-portal \
        --bin osim-migrate

# Strip debug symbols to reduce binary size (~60-80% smaller).
# The binaries are statically known here; no glob needed.
RUN strip \
        target/release/welkin-robust \
        target/release/welkin-region \
        target/release/welkin-portal \
        target/release/osim-migrate

# ============================================================================
# Stage 4: runtime — minimal production image
# ============================================================================
#
# Uses Google's distroless/cc-debian12, which provides:
#   - glibc, libgcc, libstdc++, ca-certificates
#   - NO shell, NO package manager, NO coreutils
#   - Minimal attack surface (CVE exposure reduced by ~80% vs Debian)
#
# Because there is no shell, HEALTHCHECK cannot use curl/wget.  Container
# orchestrators (Kubernetes, Docker Swarm, Nomad) should probe the HTTP
# health endpoint directly:
#
#   GET http://<host>:<port>/health  →  200 "ok"
#
# Each binary serves /health and /metrics (Prometheus) on its own HTTP port.
#
# ---------------------------------------------------------------------------
# Security considerations:
# ---------------------------------------------------------------------------
#   - Runs as UID 65534 (nonroot), the distroless convention
#   - No shell means no shell-based exploits (no /bin/sh, no /bin/bash)
#   - No package manager means no post-deploy package installation
#   - Read-only filesystem recommended (--read-only flag at runtime)
#   - SIGTERM is handled gracefully by the tokio runtime (shutdown_signal)
#   - No secrets baked in; all configuration via environment variables
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

# Re-declare the build argument so it is available in this stage.
# (Each FROM resets the ARG scope.)
ARG VERSION=0.1.0

# ---------------------------------------------------------------------------
# OCI / Docker labels
# ---------------------------------------------------------------------------
LABEL maintainer="OpenSim Rust Contributors" \
      org.opencontainers.image.title="welkin" \
      org.opencontainers.image.description="OpenSim virtual world server (Rust implementation)" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.licenses="LGPL-3.0-or-later" \
      org.opencontainers.image.source="https://github.com/opensim-rust/opensim-rust" \
      org.opencontainers.image.vendor="OpenSim Rust Contributors"

# ---------------------------------------------------------------------------
# Copy release binaries from the builder stage
# ---------------------------------------------------------------------------
# Each binary is placed in /usr/local/bin so it is on the default PATH.
# Ownership is set to nonroot (65534:65534) as a defense-in-depth measure,
# although the binaries are read-only in practice.
# ---------------------------------------------------------------------------
COPY --from=builder --chown=65534:65534 \
    /src/target/release/welkin-robust \
    /usr/local/bin/welkin-robust

COPY --from=builder --chown=65534:65534 \
    /src/target/release/welkin-region \
    /usr/local/bin/welkin-region

COPY --from=builder --chown=65534:65534 \
    /src/target/release/welkin-portal \
    /usr/local/bin/welkin-portal

COPY --from=builder --chown=65534:65534 \
    /src/target/release/osim-migrate \
    /usr/local/bin/osim-migrate

# ---------------------------------------------------------------------------
# Run as non-root user
# ---------------------------------------------------------------------------
# distroless/cc-debian12:nonroot already sets USER 65534, but we declare it
# explicitly for clarity and to survive base image tag changes.
# ---------------------------------------------------------------------------
USER 65534

# ---------------------------------------------------------------------------
# Working directory
# ---------------------------------------------------------------------------
# /home/nonroot exists in the distroless image and is writable by UID 65534.
# Region data, OAR files, and runtime state go here (via volume mounts).
# ---------------------------------------------------------------------------
WORKDIR /home/nonroot

# ---------------------------------------------------------------------------
# Default log level
# ---------------------------------------------------------------------------
# RUST_LOG controls tracing-subscriber's EnvFilter.  Examples:
#   RUST_LOG=info                      (default — recommended for production)
#   RUST_LOG=debug                     (verbose, all crates)
#   RUST_LOG=welkin_robust=debug      (verbose for one crate only)
#   RUST_LOG=warn                      (quiet — errors and warnings only)
# ---------------------------------------------------------------------------
ENV RUST_LOG=info

# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------
# distroless has no shell or HTTP client, so the Docker HEALTHCHECK
# instruction cannot be used.  Orchestrators should probe the HTTP
# health endpoint instead:
#
#   Robust:  GET http://host:8002/health
#   Region:  GET http://host:9000/health
#   Portal:  GET http://host:8080/health
#
# The /health endpoint returns HTTP 200 with body "ok" when the service
# is ready to accept traffic.
#
# The /metrics endpoint returns Prometheus-format metrics for scraping.
# ---------------------------------------------------------------------------

# ---------------------------------------------------------------------------
# Exposed ports
# ---------------------------------------------------------------------------
#   8002/tcp  — Robust grid services (login, asset, inventory, grid, etc.)
#   9000/tcp  — Region HTTP (capabilities, mesh upload, texture fetch)
#   9000/udp  — Region LLUDP (viewer protocol: avatar, object, terrain data)
#   8080/tcp  — Portal web administration interface
#
# Note: in production, run separate containers for each service rather than
# exposing all ports from a single container.  Use the CMD override to select
# which binary to run:
#
#   docker run welkin:latest welkin-robust   # port 8002
#   docker run welkin:latest welkin-region   # port 9000
#   docker run welkin:latest welkin-portal   # port 8080
# ---------------------------------------------------------------------------
EXPOSE 8002/tcp
EXPOSE 9000/tcp
EXPOSE 9000/udp
EXPOSE 8080/tcp

# ---------------------------------------------------------------------------
# Graceful shutdown
# ---------------------------------------------------------------------------
# The Rust runtime (welkin-runtime crate) installs a SIGTERM handler via
# tokio::signal.  When Docker sends SIGTERM (on `docker stop`), the server:
#
#   1. Stops accepting new connections
#   2. Drains in-flight requests (with a configurable timeout)
#   3. Flushes metrics and logs
#   4. Exits cleanly with code 0
#
# The default Docker stop timeout is 10 seconds.  For region servers with
# active viewers, increase this:
#
#   docker stop --time=30 <container>
#
# or set stop_grace_period in docker-compose.
# ---------------------------------------------------------------------------
STOPSIGNAL SIGTERM

# ---------------------------------------------------------------------------
# Entrypoint and default command
# ---------------------------------------------------------------------------
# ENTRYPOINT is empty so CMD can specify any of the four binaries.
# Default is welkin-robust (grid services), the most common deployment.
#
# Override examples:
#   docker run welkin:latest welkin-region
#   docker run welkin:latest welkin-portal
#   docker run welkin:latest osim-migrate --help
# ---------------------------------------------------------------------------
# The private service port must be reachable from sibling containers; the
# container network is the trust boundary (do not publish 8003 to the host).
ENV WELKIN_PRIVATE_BIND=0.0.0.0
ENTRYPOINT []
CMD ["welkin-robust"]
