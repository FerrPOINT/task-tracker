# Catalog health alias

## Goal

Make the Task Tracker expose the fixed Base catalog probe path `GET /health` without changing the existing public liveness endpoint `GET /api/v1/health`.

## Scope

- Add an unauthenticated `GET /health` alias returning the same liveness response as `/api/v1/health`.
- Include the alias in generated OpenAPI and the public-route security invariant.
- Add regression coverage for both paths.
- Synchronize the runtime document so historical target probe paths are not presented as current behavior.

## Verification

1. Focused API integration test fails before the alias exists, then passes after implementation.
2. Regenerate OpenAPI and run its consistency check.
3. Run backend fmt, clippy and workspace tests.
4. Rebuild the umbrella Task Tracker service, then verify `http://127.0.0.1:7721/health` and Admin Panel catalog health.
