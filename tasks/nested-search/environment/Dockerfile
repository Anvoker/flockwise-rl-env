FROM rust:1.98.1-slim-bookworm

# Miri needs a nightly toolchain. Pinned, with its sysroot built here so that
# nothing needs the network later.
ARG MIRI_NIGHTLY=nightly-2026-10-01
ENV MIRI_TOOLCHAIN=${MIRI_NIGHTLY}

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Offline documentation for the standard library.
RUN rustup component add rust-docs

RUN rustup toolchain install "${MIRI_NIGHTLY}" --profile minimal --component miri,rust-src \
    && cargo +"${MIRI_NIGHTLY}" miri setup

# Use the sysroot built above as it is. Without this, cargo-miri tries to
# rebuild it in every container and fails to swap the new one in over the
# image layer ("detected a concurrent sysroot build with different settings").
ENV MIRI_SYSROOT=/root/.cache/miri

WORKDIR /app
COPY repo/ /app/

# Fetch and build the dependencies once, and document them for offline use.
# The library doesn't compile in its starting state, so the builds are
# expected to fail after the dependencies.
RUN chmod +x ci.sh compile_checks/run.sh \
    && cargo fetch \
    && cargo doc -p rand -p rand_core -p rand_chacha \
    && (cargo test --release --no-run || true) \
    && (cargo +"${MIRI_NIGHTLY}" miri test --no-run --test miri || true)

ENV CARGO_NET_OFFLINE=true

RUN git init -q && git add -A \
    && git -c user.name=flockwise -c user.email=flockwise@example.invalid commit -q -m "Starting state"
