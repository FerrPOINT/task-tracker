# syntax=docker/dockerfile:1
# Workspace-root context: product backend and the verified sibling SDK.
FROM rust:1.88.0-slim-bookworm AS manifests
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
COPY task-tracker/backend/Cargo.toml task-tracker/backend/Cargo.lock* ./
COPY task-tracker/backend/api/Cargo.toml api/Cargo.toml
COPY task-tracker/backend/app/Cargo.toml app/Cargo.toml
COPY task-tracker/backend/domain/Cargo.toml domain/Cargo.toml
COPY task-tracker/backend/infra/Cargo.toml infra/Cargo.toml
COPY task-tracker/backend/shared/Cargo.toml shared/Cargo.toml
COPY task-tracker/backend/server/Cargo.toml server/Cargo.toml
COPY task-tracker/backend/cli/Cargo.toml cli/Cargo.toml
COPY task-tracker/backend/migration/Cargo.toml migration/Cargo.toml
# Product metadata is stable; actual SDK bytes remain an independent layer input.
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} \
    for package in api app domain infra shared server cli migration; do mkdir -p "$package/src"; printf '%s\n' '' > "$package/src/lib.rs"; printf '%s\n' 'fn main() {}' > "$package/src/main.rs"; done \
    && mkdir -p api/src/bin && printf '%s\n' 'fn main() {}' > api/src/bin/gen_openapi.rs

FROM manifests AS unit-dependencies
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked \
    cargo test --locked --release -p domain --lib --no-run \
    && cargo clean --release -p api -p app -p domain -p infra -p shared -p server -p task-tracker-cli -p migration

FROM unit-dependencies AS dependencies
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked \
    cargo build --locked --release -p server --bin server \
    && cargo clean --release -p api -p app -p domain -p infra -p shared -p server -p task-tracker-cli -p migration

FROM unit-dependencies AS unit
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} rm -rf -- shared/src domain/src
COPY task-tracker/backend/shared ./shared
COPY task-tracker/backend/domain ./domain
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked cargo test --locked --release -p domain --lib && printf 'passed\n' > /tmp/rust-unit-check

FROM dependencies AS builder
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=bind,from=unit,source=/tmp/rust-unit-check,target=/tmp/rust-unit-check test "$(cat /tmp/rust-unit-check)" = passed
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} rm -rf -- api/src app/src domain/src infra/src shared/src server/src migration/src
COPY task-tracker/backend/api ./api
COPY task-tracker/backend/app ./app
COPY task-tracker/backend/domain ./domain
COPY task-tracker/backend/infra ./infra
COPY task-tracker/backend/shared ./shared
COPY task-tracker/backend/server ./server
COPY task-tracker/backend/migration ./migration
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,target=/usr/local/cargo/registry,id=${CARGO_REGISTRY_CACHE_ID}-${TARGETARCH},sharing=locked cargo build --locked --release -p server --bin server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 wget && rm -rf /var/lib/apt/lists/*
RUN groupadd -r tasktracker && useradd -r -g tasktracker -d /app -s /usr/sbin/nologin tasktracker
WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/task-tracker-server
RUN mkdir -p /var/lib/tasktracker/uploads && chown -R tasktracker:tasktracker /var/lib/tasktracker
USER tasktracker
ENV TASKTRACKER_UPLOADS_DIR=/var/lib/tasktracker/uploads
EXPOSE 3456
CMD ["task-tracker-server"]
