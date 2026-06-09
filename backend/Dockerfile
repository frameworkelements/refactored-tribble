# ---- Build stage ---------------------------------------------------------
FROM rust:1-bookworm AS builder

WORKDIR /build

# Cache dependencies: copy manifests first, build a dummy target, then the
# real sources. This keeps `cargo build` from re-downloading crates on every
# source change.
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
# Touch main so cargo notices the real sources, then build.
RUN touch src/main.rs && cargo build --release

# ---- Runtime stage -------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates for outbound TLS; curl for the container healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged, non-root user.
RUN useradd --system --uid 10001 --user-group --no-create-home appuser

COPY --from=builder /build/target/release/lms-backend /usr/local/bin/lms-backend

USER appuser
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/lms-backend"]
