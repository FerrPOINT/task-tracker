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

### 12.1 Pre-commit

```bash
# .husky/pre-commit
pnpm lint-staged
```

```json
// package.json
"lint-staged": {
  "*.{ts,tsx}": ["eslint --fix", "prettier --write"],
  "*.rs": ["rustfmt"]
}
```

### 12.2 Pre-push (optional)

```bash
# .husky/pre-push
cargo test --lib
pnpm vitest run
```

## 13. Communication

- Issues: GitHub issues.
- Discussions: GitHub discussions.
- Russian or English accepted.

## 14. License

See repository license file.
