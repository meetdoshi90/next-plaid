# =============================================================================
# Next-Plaid API Dockerfile
# =============================================================================
# Embeddings-only REST API for multi-vector search.
# You provide pre-computed ColBERT embeddings; the server handles indexing,
# search, and metadata filtering.
#
# Build:
#   docker build -t next-plaid-api -f next-plaid-api/Dockerfile .
#
# Run:
#   docker run -p 8080:8080 -v ~/.local/share/next-plaid:/data/indices next-plaid-api
#
# Vector Database Persistence:
#   Default location: ~/.local/share/next-plaid/<index-name>/
#   Mount any host path to /data/indices to persist indices across restarts.
#
# Environment variables (optional):
#   RUST_LOG: Log level (default: info)
#   RATE_LIMIT_ENABLED: Enable rate limiting (default: false)
#   RATE_LIMIT_PER_SECOND: Max requests/s when rate limiting is enabled (default: 50)
#   RATE_LIMIT_BURST_SIZE: Burst size when rate limiting is enabled (default: 100)
#   CONCURRENCY_LIMIT: Max concurrent in-flight requests (default: 100)
#   MAX_QUEUED_TASKS_PER_INDEX: Max pending updates per index (default: 10)
#   MAX_BATCH_DOCUMENTS: Documents per update batch (default: 300)
#   BATCH_CHANNEL_SIZE: Update queue buffer size per index (default: 100)
#   DELETE_BATCH_MIN_WAIT: Min ms before processing deletes (default: 500)
#   DELETE_BATCH_MAX_WAIT: Max ms for delete batching (default: 2000)
#   MAX_DELETE_BATCH_CONDITIONS: Max delete conditions per batch (default: 200)
# =============================================================================

# =============================================================================
# Chef stage - Install cargo-chef for dependency caching
# =============================================================================
FROM debian:bookworm-slim AS chef

RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    libopenblas-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust and cargo-chef
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN cargo install cargo-chef

WORKDIR /app

# =============================================================================
# Planner stage - Create dependency recipe
# =============================================================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY next-plaid ./next-plaid
COPY next-plaid-api ./next-plaid-api

RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Builder stage - Build dependencies (cached) then application
# =============================================================================
FROM chef AS builder

# Build dependencies first (this layer is cached if recipe.json doesn't change)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --package next-plaid-api --features openblas

# Now build the actual application
COPY Cargo.toml Cargo.lock ./
COPY next-plaid ./next-plaid
COPY next-plaid-api ./next-plaid-api

RUN cargo build --release --package next-plaid-api --features openblas

# =============================================================================
# Runtime stage
# =============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    libopenblas0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash nextplaid

# Create directory for indices
RUN mkdir -p /data/indices && chown -R nextplaid:nextplaid /data

# Copy binary from builder
COPY --from=builder /app/target/release/next-plaid-api /usr/local/bin/next-plaid-api

# Switch to non-root user
USER nextplaid

# Expose API port
EXPOSE 8080

# Default environment variables
ENV RUST_LOG=info
ENV INDEX_DIR=/data/indices
# Prevent OpenBLAS thread explosion when used alongside rayon parallelism
ENV OPENBLAS_NUM_THREADS=1

# Health check
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=2 \
    CMD curl -f --max-time 5 http://localhost:8080/health || exit 1

ENTRYPOINT ["next-plaid-api"]
CMD ["--host", "0.0.0.0", "--port", "8080", "--index-dir", "/data/indices"]