# syntax=docker/dockerfile:1.27.1@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Umbrella build for the SDLC fleet root compose (context = /opt/dev/sdlc).
# Stages the sibling services-base frontend so the `@sdlc/ui` file dependency
# (../../services-base/frontend) resolves.
FROM node:22.20.0-bookworm-slim@sha256:b21fe589dfbe5cc39365d0544b9be3f1f33f55f3c86c87a76ff65a02f8f5848e AS toolchain
ARG PDLC_RUN_SLOT_SHARING=locked
ARG TARGETARCH
ARG PNPM_CACHE_ID=pdlc-operator-downloads-v1-pnpm10-node22

RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} corepack enable && corepack prepare pnpm@10.28.1 --activate

WORKDIR /app

FROM toolchain AS install-manifest
ARG PDLC_RUN_SLOT_SHARING=locked
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=bind,source=task-tracker/frontend/package.json,target=/tmp/product-package.json node -e 'const fs = require("node:fs"); const pkg = JSON.parse(fs.readFileSync("/tmp/product-package.json", "utf8")); if (!pkg || Array.isArray(pkg) || typeof pkg !== "object") throw new Error("Package manifest must be an object"); delete pkg.scripts; fs.writeFileSync("package.json", JSON.stringify(pkg) + "\n");'

FROM toolchain AS dependencies
ARG PDLC_RUN_SLOT_SHARING=locked
COPY --from=install-manifest /app/package.json ./
COPY task-tracker/frontend/pnpm-lock.yaml task-tracker/frontend/pnpm-workspace.yaml ./
COPY services-base/frontend /services-base/frontend
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=cache,id=${PNPM_CACHE_ID}-${TARGETARCH},target=/pnpm/store,sharing=locked pnpm install --store-dir=/pnpm/store --frozen-lockfile --ignore-scripts


FROM dependencies AS generated
ARG PDLC_RUN_SLOT_SHARING=locked
COPY task-tracker/frontend/package.json ./
COPY task-tracker/openapi/openapi.json ../openapi/openapi.json
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} pnpm run postinstall && test -s src/api/generated.ts

FROM dependencies AS unit
ARG PDLC_RUN_SLOT_SHARING=locked
COPY task-tracker/frontend/nginx.conf ./nginx.conf
COPY --exclude=Dockerfile* --exclude=**/Dockerfile* --exclude=.dockerignore --exclude=**/.dockerignore --exclude=nginx.conf --exclude=**/nginx.conf task-tracker/frontend/ ./
COPY --from=generated /app/src/api/generated.ts ./src/api/generated.ts
ARG VITE_NAMESPACE_ENABLED="false"
ENV VITE_NAMESPACE_ENABLED=${VITE_NAMESPACE_ENABLED}
ARG VITE_PLATFORM_BRANDING_URL=""
ENV VITE_PLATFORM_BRANDING_URL=${VITE_PLATFORM_BRANDING_URL}
ARG VITE_PLATFORM_SERVICES_URL=""
ENV VITE_PLATFORM_SERVICES_URL=${VITE_PLATFORM_SERVICES_URL}
ARG VITE_AUTH_ISSUER="http://localhost:7701"
ENV VITE_AUTH_ISSUER=${VITE_AUTH_ISSUER}
ARG VITE_PLATFORM_ORIGIN=""
ENV VITE_PLATFORM_ORIGIN=${VITE_PLATFORM_ORIGIN}
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} pnpm test:unit && pnpm exec tsc --noEmit && printf 'passed\n' > /tmp/frontend-unit-check

FROM dependencies AS builder
ARG PDLC_RUN_SLOT_SHARING=locked
COPY --exclude=Dockerfile* --exclude=**/Dockerfile* --exclude=.dockerignore --exclude=**/.dockerignore --exclude=nginx.conf --exclude=**/nginx.conf --exclude=**/*.test.* --exclude=**/*.spec.* --exclude=**/__tests__/** --exclude=vitest.config.ts --exclude=**/vitest.config.ts --exclude=**/test/setup.ts task-tracker/frontend/ ./
COPY --from=generated /app/src/api/generated.ts ./src/api/generated.ts
ARG VITE_NAMESPACE_ENABLED="false"
ENV VITE_NAMESPACE_ENABLED=${VITE_NAMESPACE_ENABLED}
ARG VITE_PLATFORM_BRANDING_URL=""
ENV VITE_PLATFORM_BRANDING_URL=${VITE_PLATFORM_BRANDING_URL}
ARG VITE_PLATFORM_SERVICES_URL=""
ENV VITE_PLATFORM_SERVICES_URL=${VITE_PLATFORM_SERVICES_URL}
ARG VITE_AUTH_ISSUER="http://localhost:7701"
ENV VITE_AUTH_ISSUER=${VITE_AUTH_ISSUER}
ARG VITE_PLATFORM_ORIGIN=""
ENV VITE_PLATFORM_ORIGIN=${VITE_PLATFORM_ORIGIN}
# A changed unit file reruns checks; identical production inputs keep compilation cached.
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} --mount=type=bind,from=unit,source=/tmp/frontend-unit-check,target=/tmp/frontend-unit-check test "$(cat /tmp/frontend-unit-check)" = passed
# The required unit stage already typechecks these production sources with noEmit.
RUN --mount=type=cache,id=pdlc-host-run-slot-v1,target=/.__pdlc_build_slot__,sharing=${PDLC_RUN_SLOT_SHARING} pnpm exec vite build


FROM nginx:alpine@sha256:72aeb3308ef6cb3118242fb16389518923916dfa206e32d1a8377acc328b9870
ARG PDLC_RUN_SLOT_SHARING=locked
ENV NGINX_ENTRYPOINT_WORKER_PROCESSES_AUTOTUNE=1
ARG VITE_NAMESPACE_ENABLED="false"
LABEL io.pdlc.namespace-ui.enabled=${VITE_NAMESPACE_ENABLED}
COPY --from=builder /app/dist /usr/share/nginx/html
COPY task-tracker/frontend/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
