FROM rust:1.86-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY backend/Cargo.toml backend/Cargo.lock* ./
COPY backend/shared ./shared
COPY backend/domain ./domain
COPY backend/infra ./infra
COPY backend/app ./app
COPY backend/api ./api
COPY backend/server ./server
COPY backend/cli ./cli

RUN cargo build --release --bin task-tracker-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/task-tracker-server /usr/local/bin/task-tracker-server
EXPOSE 3456
CMD ["task-tracker-server"]
