# syntax=docker/dockerfile:1.6

# ---- Build stage ----
FROM rust:1.78-bookworm AS builder

WORKDIR /app

# Build dependencies in a separate layer for better caching.
# We need at least a dummy Cargo.toml + main.rs to do this.
RUN mkdir src && echo "fn main() {}" > src/main.rs

COPY Cargo.toml ./
RUN cargo build --release || true

# Now copy the real source and build the actual binary.
COPY src/ ./src/

# Touch the source so cargo rebuilds with the real code, not the dummy.
RUN touch src/main.rs && cargo build --release --bin search-api

# ---- Runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN groupadd -r app && useradd -r -g app -d /app app

COPY --from=builder /app/target/release/search-api /usr/local/bin/search-api

USER app

EXPOSE 8080

ENV PORT=8080
ENV RUST_LOG=info

CMD ["search-api"]
