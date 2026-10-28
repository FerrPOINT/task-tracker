# Umbrella build for the SDLC fleet root compose (context = /opt/dev/sdlc).
# Stages the sibling services-base checkout so the pdlc-auth-core path
# dependency (../../../services-base/...) resolves.
FROM rust:1.88.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev unzip curl unzip && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# api/Cargo.toml: pdlc-auth-core = { path = "../../../services-base/..." }
# /app/api/../../../services-base == /services-base
COPY services-base/Cargo.toml /services-base/Cargo.toml
COPY services-base/base-modules/rust/pdlc-telemetry /services-base/base-modules/rust/pdlc-telemetry
COPY services-base/base-modules/rust/pdlc-auth-core /services-base/base-modules/rust/pdlc-auth-core
COPY services-base/base-modules/rust/pdlc-cli-core/Cargo.toml /services-base/base-modules/rust/pdlc-cli-core/Cargo.toml
COPY services-base/base-modules/rust/pdlc-shared /services-base/base-modules/rust/pdlc-shared
COPY task-tracker/backend/Cargo.toml task-tracker/backend/Cargo.lock* ./
COPY task-tracker/backend/shared ./shared
COPY task-tracker/backend/domain ./domain
COPY task-tracker/backend/infra ./infra
COPY task-tracker/backend/migration ./migration
COPY task-tracker/backend/app ./app
COPY task-tracker/backend/api ./api
COPY task-tracker/backend/server ./server
COPY task-tracker/backend/cli/Cargo.toml cli/Cargo.toml

ARG CARGO_BUILD_JOBS=1
RUN mkdir -p /services-base/base-modules/rust/pdlc-cli-core/src cli/src \
    && printf '%s\n' 'compile_error!("metadata-only pdlc-cli-core must not be compiled");' > /services-base/base-modules/rust/pdlc-cli-core/src/lib.rs \
    && printf '%s\n' 'compile_error!("metadata-only task-tracker CLI must not be compiled");' > cli/src/main.rs \
    && cargo test --locked --release -p domain --lib \
    && cargo build --locked --release -p server --bin server

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
