# Task Tracker README Evidence Plan

> **Status 2026-09-18:** historical record of the initial README migration. The active Base contract is desktop-only; mobile proof and auth-screen evidence stay in product UI QA, not the root README.

## Evidence Decision

- Use the existing `10-reports.png`: it is a synthetic `Demo Project` report with no credentials, personal data, URLs, actual IDs or timestamps.
- Do not place narrow-viewport captures in the root README or manifest; responsive behavior remains product UI QA.
- Do not place `01-login.png` in the root README: it advertises the MVP/demo "any login/password" behavior. Do not use dashboard or issue-detail images because they carry more fixture and workflow context than the root entry point needs.

## Execution

1. Add a test-first README validator for anchors, reviewed evidence, local images, placeholders and local path leaks.
2. Add an independent `readme` GitHub Actions job.
3. Replace the repeated screenshot matrix with two reviewed evidence sections and link the complete screenshot inventory in `docs/screenshots/`.
4. Add a local blue/teal tracker banner and an evidence note in the current README migration plan.
5. Verify backend/frontend gates, real Compose health and metrics, browser E2E, dimensions, local validator and hosted CI before publishing.
