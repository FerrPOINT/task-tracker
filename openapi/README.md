# OpenAPI-first workflow

1. Backend is the source of truth for the API schema.
2. Rust handlers and DTOs use `utoipa` (`#[derive(ToSchema)]`, `#[utoipa::path]`).
3. `cargo run --bin openapi-gen` writes `openapi/openapi.json` without starting a server.
4. `pnpm generate:api` consumes `openapi/openapi.json` and writes `frontend/src/api/generated.ts`.
5. Frontend uses `openapi-fetch` with the generated types (`paths`, `components`).
6. `pnpm build` regenerates the client automatically before `tsc` and `vite build`.

## Commands

```bash
# Regenerate OpenAPI schema from backend
cd /opt/dev/task-tracker/backend
cargo build --bin openapi-gen
./target/debug/openapi-gen /opt/dev/task-tracker/openapi/openapi.json

# Regenerate frontend client
cd /opt/dev/task-tracker/frontend
pnpm generate:api

# Full frontend checks
pnpm typecheck && pnpm test -- --run && pnpm build

# Start backend + DB
cd /opt/dev/task-tracker
docker compose up -d postgres redis backend
```

## Notes

- Rust controller interfaces are handwritten; they are the source of truth, not generated.
- The generated `frontend/src/api/generated.ts` is committed to keep builds hermetic but should be regenerated whenever backend schemas change.
- Add `VITE_API_BASE_URL=http://127.0.0.1:3456/api/v1` to `frontend/.env` for local dev.
