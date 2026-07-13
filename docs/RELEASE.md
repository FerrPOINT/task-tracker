# Release Process — Task Tracker

## 1. Overview

Как выпускать релизы Task Tracker: versioning, branching, testing, tagging, deployment.

## 2. Versioning

Semantic Versioning: `MAJOR.MINOR.PATCH`.

| Bump | When |
|------|------|
| MAJOR | Breaking API change |
| MINOR | New feature, backward compatible |
| PATCH | Bugfix, security patch |

## 3. Release Branches

- `main` — always deployable.
- `release/vX.Y.Z` — stabilization branch.
- Tags: `vX.Y.Z`.

## 4. Release Checklist

- [ ] `CHANGELOG.md` updated.
- [ ] Version bumped in `backend/Cargo.toml`.
- [ ] Version bumped in `frontend/package.json`.
- [ ] API version unchanged unless breaking.
- [ ] All tests green.
- [ ] E2E critical path green.
- [ ] Security scan green.
- [ ] Docker images built and pushed.
- [ ] Migration tested on staging.
- [ ] Tag created.

## 5. Staging Release

```bash
git checkout -b release/v0.2.0
# final fixes
./scripts/deploy-staging.sh
# QA sign-off
```

## 6. Production Release

```bash
git tag v0.2.0
git push origin v0.2.0
./scripts/deploy-production.sh
```

## 7. Hotfix

```bash
git checkout -b hotfix/v0.2.1 main
# fix
PR → main and cherry-pick to release branch
```

## 8. Rollback Release

```bash
git checkout v0.1.9
./scripts/deploy-production.sh --tag v0.1.9
```

## 9. Communication

- Release notes on GitHub.
- Announce in #task-tracker.
- Update documentation if needed.

## 10. Post-Release

- Monitor errors/metrics 24h.
- Collect feedback.
- Plan next release.

## References

- `CHANGELOG.md` (корень репозитория)
- `CONTRIBUTING.md` (корень репозитория)
- `docs/DEPLOYMENT.md`
- `docs/OPS_RUNBOOK.md`
