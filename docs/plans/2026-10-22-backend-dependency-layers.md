# Стабильные backend layers и минимальный context PDLC3

Область: две backend Docker recipes, их собственные dockerignore, один source
contract guard и обычные tooling/CI callers. Полный план и общий контракт —
[Base BUILD_PERFORMANCE](../../../services-base/docs/BUILD_PERFORMANCE.md).

Сначала сохранить actual source/SDK/runtime contract, затем разделить preparation:
metadata и product stubs → domain test dependencies → server dependencies.
Unit копирует настоящие Shared/Domain и не наследует server graph. Full builder
требует successful unit marker; все product stubs очищаются после preparation,
чтобы старый timestamp не оставлял stub binary. Compiled target остаётся внутри
immutable layer, downloads используют общий architecture-qualified Cargo188
cache. Jobs2 — безопасный исходный ARG, fastest profile ещё не выбран.

Dockerfile-specific context включает только собственный backend и прежний SDK
COPY closure. Сохраняются все существующие private/cache/report exclusions,
Base/UI/skills/operator pins, Cargo/features/toolchains, runtime image version
и packages, users/ownership/ports/volumes/migrations. CLI и OpenAPI generators
не становятся случайными server targets; их исходные entrypoints не меняются.

Проверки: causal RED/GREEN source graph, реальные manifest/stub/consumer paths,
context policy и сохранённые runtime bytes; штатный scoped Fast tooling/docs.
Actual Docker native gate (old timestamp, cache invalidation/loss, units,
artifact/runtime identity, CPU/RAM/wall/context bytes) остаётся NOT_RUN до
доступного общего host slot. Не запускать full matrix по каждой recipe правке.
