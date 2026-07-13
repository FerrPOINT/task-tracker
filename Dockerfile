# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:24.18.0-alpine AS frontendbuilder
WORKDIR /build
ENV PNPM_CACHE_FOLDER=.cache/pnpm/
ENV PUPPETEER_SKIP_DOWNLOAD=true
ENV CYPRESS_INSTALL_BINARY=0
COPY frontend/pnpm-lock.yaml frontend/package.json frontend/pnpm-workspace.yaml ./
RUN npm install -g corepack && corepack enable && pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm run build

FROM golang:1.26-alpine AS apibuilder
WORKDIR /build
COPY . ./
COPY --from=frontendbuilder /build/dist ./frontend/dist
RUN go build -o task-tracker .

FROM alpine:latest
RUN apk add --no-cache ca-certificates
WORKDIR /app
EXPOSE 19876
ENV TASKTRACKER_SERVICE_ROOTPATH=/app/
ENV TASKTRACKER_SERVICE_INTERFACE=:19876
COPY --from=apibuilder /build/task-tracker /app/task-tracker
ENTRYPOINT ["/app/task-tracker"]
