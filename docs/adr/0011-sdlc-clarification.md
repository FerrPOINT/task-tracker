# ADR-0011: Strict SDLC Ownership And Durable Clarification Gate

## Status

Accepted for the approved Tracker backend slice. Deployment acceptance pending.

## Context

Fleet forwards the original verified central bearer. Tracker's existing Central
Auth project bypass cannot authorize task-owner decisions. A saved answer must
survive restart and delivery failures without becoming implicit requirements
consent or duplicating a Workflow resume.

## Decision

Use a dedicated strict central middleware and PostgreSQL subject/membership
checks, independent of legacy Tracker policy. Store immutable instance/task/
project/root/owner binding, versioned question history and full immutable
requirements. Only the assigned scoped PM publishes; a separately trusted
verifier attests known checks against exact revision/hash. Only the human owner
session confirms. Question publication invalidates readiness until PM publishes
a new document after answers.

Lock issue then task, recheck authorization, and commit aggregate state, history,
canonical idempotency result, issue status history and outbox together. Add a
database issue trigger to prevent legacy transitions/sprint updates from bypassing
confirmation. Preserve append-only history and refuse destructive down migration.
Expose an authorized task-scoped pull outbox with stable event IDs for Fleet.

## Consequences

SDLC provisioning needs stable instance configuration, explicit machine project
memberships, scoped current assignment grants and an independent verifier.
JSONB aggregate supplies the current snapshot; relational history preserves
immutable records and supports audit/deduplication. Reads serialize on the task
lock in this bounded slice. Fleet owns inbox/cursor/projection and Workflow owns
checkpoint/rebind/resume; Tracker event persistence does not claim delivery.
No real PM or production acceptance follows from fixture integration tests.

See [contract](../CHAT_CLARIFICATION_CONTRACT.md) and
[verification](../CHAT_CLARIFICATION_VERIFICATION.md).
