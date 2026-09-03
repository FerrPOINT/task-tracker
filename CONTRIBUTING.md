# Contributing — Task Tracker

## 1. Getting Started

```bash
git clone git@github.com:FerrPOINT/task-tracker.git
cd task-tracker
cp .env.example .env
```

## 2. Development Setup

```bash
# Backend
cd backend
cargo build
cargo run --bin server

# Frontend
cd frontend
pnpm install
pnpm dev
```

## 3. Before You Contribute

- Read `docs/ARCHITECTURE.md`.
- Check `docs/CODE_STYLE.md`.
- Ensure your change is covered by docs/ADR.md if it changes architecture.
- Open an issue or discuss in existing issue before large changes.

## 4. Making Changes

1. Create branch: `feat/short-desc` or `fix/short-desc`.
2. Write code following CODE_STYLE.md.
3. Add/update tests.
4. Update docs if needed.
5. Run checks locally.

## 5. Local Checks

```bash
# Backend
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# Frontend
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
```

## 6. Commit Messages

Conventional commits:

```
feat(issues): add worklog CRUD
fix(auth): refresh cookie path
docs(api): add WebSocket payloads
refactor(board): split BoardColumn component
test(e2e): cover issue transition
```

## 7. Pull Request

- Small PRs (max 500 lines).
- Self-review first.
- Fill PR template.
- Link related issue.
- Ensure CI green.
- Address review feedback.

## 8. Code Review

- One approve required.
- Owner of relevant area should review.
- No merge without CI green.

## 10. Documentation Updates

Каждый PR должен обновлять документацию при изменении:

- архитектуры, API, workflow — обновить соответствующий `docs/*.md`.
- нового env или настройки — обновить `docs/DEPLOYMENT.md`, `.env.example`, `README.md`.
- нового endpoint — обновить `docs/API.md` и OpenAPI.
- нового компонента — обновить `docs/UI_UX.md` или `docs/FRONTEND_ARCHITECTURE.md`.

## 11. Release

- Maintainers cut releases.
- Follow Semantic Versioning.
- Update CHANGELOG.md before tagging.

## 12. Pre-commit / Pre-push

Hooks are managed by [lefthook](https://github.com/evilmartians/lefthook).
The configuration lives in `lefthook.yml` at the repository root.

### 12.1 Pre-commit

Runs in parallel on staged files:

- `cargo fmt --all -- --check` (Rust files)
- `cargo clippy --workspace --all-targets` (Rust files)
- `pnpm typecheck` (TS/TSX files)
- `pnpm test -- --run` (TS/TSX files)
- `pnpm exec eslint . --max-warnings=0` (TS/TSX files)

### 12.2 Pre-push

Runs before push to origin:

- `cargo test --workspace -- --test-threads=1`
- `pnpm build` (frontend production build)
- `pnpm exec playwright test --project=chromium --grep="smoke"` (E2E smoke)

### 12.3 Commit message

Enforces [Conventional Commits](https://www.conventionalcommits.org/):
`feat|fix|docs|style|refactor|test|chore|perf|ci|build(scope)!: description`

## 13. Communication

- Issues: GitHub issues.
- Discussions: GitHub discussions.
- Russian or English accepted.

## 14. License and Contribution Rights

This project is proprietary source-available, not open source. A PR is accepted only if the contributor grants FerrPOINT an irrevocable, worldwide, perpetual, royalty-free, sublicensable, transferable right to use, reproduce, modify, distribute, relicense, commercialize, and sell the contribution as part of the software or related products and services.

If you do not agree to this grant of rights, do not submit a PR, patch, documentation change, design, review suggestion, or other contribution.

## 15. References

- `docs/ARCHITECTURE.md` — общая архитектура и стек.
- `docs/CODE_STYLE.md` — стиль кода Rust / TypeScript.
- `docs/TESTING.md` — стратегия тестирования и fixtures.
- `docs/ADR.md` — архитектурные решения.
- `docs/SECURITY.md` — политики безопасности.
- `docs/CI_CD.md` — pipeline и качество кода.
