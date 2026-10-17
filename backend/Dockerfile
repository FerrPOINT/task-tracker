# syntax=docker/dockerfile:1.27.1@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Standalone product Compose build; context is the parent source workspace.
# Keep the verified sibling Base sources at their Cargo path-dependency location.
FROM rust:1.88.0-slim-bookworm@sha256:38bc5a86d998772d4aec2348656ed21438d20fcdce2795b56ca434cf21430d89 AS dependencies
ARG PDLC_RUN_SLOT_SHARING=locked
ARG TARGETARCH
ARG CARGO_REGISTRY_CACHE_ID=pdlc-operator-downloads-v1-cargo188
ARG CARGO_BUILD_JOBS=2

RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} apt-get update && apt-get install -y pkg-config libssl-dev unzip curl unzip && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# api/Cargo.toml: sdlc-auth-core = { path = "../../../services-base/..." }
# /app/api/../../../services-base == /services-base
COPY services-base/Cargo.toml /services-base/Cargo.toml
COPY services-base/crates/sdlc-telemetry /services-base/crates/sdlc-telemetry
COPY services-base/crates/sdlc-auth-core /services-base/crates/sdlc-auth-core
COPY services-base/crates/sdlc-cli-core /services-base/crates/sdlc-cli-core
COPY services-base/crates/sdlc-shared /services-base/crates/sdlc-shared
COPY task-tracker/backend/api/Cargo.toml api/Cargo.toml
COPY task-tracker/backend/app/Cargo.toml app/Cargo.toml
COPY task-tracker/backend/Cargo.toml task-tracker/backend/Cargo.lock* ./
COPY task-tracker/backend/cli/Cargo.toml cli/Cargo.toml
COPY task-tracker/backend/domain/Cargo.toml domain/Cargo.toml
COPY task-tracker/backend/infra/Cargo.toml infra/Cargo.toml
COPY task-tracker/backend/migration/Cargo.toml migration/Cargo.toml
COPY task-tracker/backend/server/Cargo.toml server/Cargo.toml
COPY task-tracker/backend/shared/Cargo.toml shared/Cargo.toml
# Immutable dependency layer; all product packages are removed before real source COPY.
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked \
    for package in api app cli domain infra migration server shared; do mkdir -p "$package/src"; printf '%s\n' '' > "$package/src/lib.rs"; printf '%s\n' 'fn main() {}' > "$package/src/main.rs"; done \
    && mkdir -p api/src/bin \
    && printf '%s\n' 'fn main() {}' > api/src/bin/gen_openapi.rs \
    && cargo build --locked --release --bin server \
    && cargo clean --release -p api -p app -p task-tracker-cli -p domain -p infra -p migration -p server -p shared

FROM dependencies AS unit-dependencies
ARG PDLC_RUN_SLOT_SHARING=locked
# Compile the test dependency graph before mutable product sources.
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked \
    cargo test --locked --release -p domain --lib --no-run \
    && cargo clean --release -p api -p app -p task-tracker-cli -p domain -p infra -p migration -p server -p shared

FROM unit-dependencies AS unit
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} rm -rf -- domain/src shared/src
ARG TARGETARCH
ARG CARGO_REGISTRY_CACHE_ID=pdlc-operator-downloads-v1-cargo188
COPY task-tracker/backend/domain ./domain
COPY task-tracker/backend/shared ./shared
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked cargo test --locked --release -p domain --lib && printf 'passed\n' > /tmp/rust-unit-check

FROM dependencies AS builder
ARG PDLC_RUN_SLOT_SHARING=locked
ARG TARGETARCH
ARG CARGO_REGISTRY_CACHE_ID=pdlc-operator-downloads-v1-cargo188
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=bind,from=unit,source=/tmp/rust-unit-check,target=/tmp/rust-unit-check test "$(cat /tmp/rust-unit-check)" = passed
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} rm -rf -- api/src app/src cli/src infra/src migration/src server/src domain/src shared/src

COPY task-tracker/backend/Cargo.toml task-tracker/backend/Cargo.lock* ./
COPY task-tracker/backend/shared ./shared
COPY task-tracker/backend/domain ./domain
COPY task-tracker/backend/infra ./infra
COPY task-tracker/backend/migration ./migration
COPY task-tracker/backend/app ./app
COPY task-tracker/backend/api ./api
COPY task-tracker/backend/server ./server
COPY task-tracker/backend/cli ./cli

RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked \
    cargo build --locked --release --bin server \
    && install -m 0755 target/release/server /app/task-tracker-server

FROM debian:bookworm-slim@sha256:7c7b2c966bc9ee8cedfeef67e0e279108992c77681fa595db4a9d65c06ccc587
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} apt-get update && apt-get install -y ca-certificates libssl3 wget && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} groupadd -r tasktracker && useradd -r -g tasktracker -d /app -s /usr/sbin/nologin tasktracker
WORKDIR /app
COPY --from=builder /app/task-tracker-server /usr/local/bin/task-tracker-server
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} test -f /usr/local/bin/task-tracker-server && test -x /usr/local/bin/task-tracker-server \
    && mkdir -p /var/lib/tasktracker/uploads && chown -R tasktracker:tasktracker /var/lib/tasktracker
USER tasktracker
ENV TASKTRACKER_UPLOADS_DIR=/var/lib/tasktracker/uploads
EXPOSE 3456
CMD ["task-tracker-server"]
