# syntax=docker/dockerfile:1.27.1@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Umbrella build for the SDLC fleet root compose (context = /opt/dev/sdlc).
# Stages the sibling services-base checkout so the sdlc-auth-core path
# dependency (../../../services-base/...) resolves.
FROM rust:1.88.0-slim-bookworm AS builder
ARG PDLC_RUN_SLOT_SHARING=locked

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
COPY task-tracker/backend/shared ./shared
COPY task-tracker/backend/domain ./domain
COPY task-tracker/backend/infra ./infra
COPY task-tracker/backend/migration ./migration
COPY task-tracker/backend/app ./app
COPY task-tracker/backend/api ./api
COPY task-tracker/backend/server ./server
COPY task-tracker/backend/cli ./cli

RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} cargo test --locked --release -p domain --lib && cargo build --locked --release --bin server

FROM debian:bookworm-slim
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} apt-get update && apt-get install -y ca-certificates libssl3 wget && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} groupadd -r tasktracker && useradd -r -g tasktracker -d /app -s /usr/sbin/nologin tasktracker
WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin/task-tracker-server
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} mkdir -p /var/lib/tasktracker/uploads && chown -R tasktracker:tasktracker /var/lib/tasktracker
USER tasktracker
ENV TASKTRACKER_UPLOADS_DIR=/var/lib/tasktracker/uploads
EXPOSE 3456
CMD ["task-tracker-server"]
