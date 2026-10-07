# Backup & Restore — Task Tracker

## Контракт

`scripts/backup.sh` и `scripts/restore.sh` делегируют операции
`services-base/scripts/platform_backup.py`. Это согласованный архив полного
workspace, а не отдельный dump Task Tracker. Состав, проверки checksum,
права БД, файловые volumes и блокировки определяет Base по явному профилю.
Имена физических ресурсов не вычисляются в Task Tracker.

Обязательные переменные для обеих команд:

| Переменная | Значение |
|---|---|
| `SDLC_TASK` | Владелец операции, без секретов |
| `SDLC_WORKSPACE_DIR` | Абсолютный путь к подготовленному workspace с Base |
| `SDLC_PROJECT` | Логический workspace: `sdlc1` или `sdlc2` |
| `SDLC_DOCKER_CONTEXT` | Явный Docker context нужного daemon |
| `SDLC_SIGNING_KEY` | Путь к сохранённому ключу Central Auth |

Команды требуют ровно один аргумент — полный путь к архиву. Профиль читается
из `$SDLC_WORKSPACE_DIR/workspace.local.json`, app Compose — из
`$SDLC_WORKSPACE_DIR/docker-compose.local.yml`. Инструменты и подготовка
назначения описаны в Base `deploy/LOCAL_GROUPS.md`.

## Резервная копия

После выбора исходного workspace и защищённого места хранения:

```bash
./scripts/backup.sh /protected/backups/workspace-2026-10-08.tar.gz
```

Передаётся `--quiesce`: Base согласованно приостанавливает writers и возвращает
их согласно своему протоколу. Планируйте окно операции; wrapper не запускает
контейнеры напрямую и не удаляет старые архивы. Ключ не включается в архив:
сохраняется его fingerprint, сам ключ хранится отдельно в защищённом месте.

## Восстановление

Сначала по процедуре Base подготовьте отдельное изолированное пустое
назначение, совместимые образы и сохранённые секреты. Затем выберите
**профиль назначения**, а не рабочий workspace, в `SDLC_WORKSPACE_DIR`;
`SDLC_PROJECT` остаётся логическим `sdlc1`/`sdlc2`. Физические временные проекты
берутся из квалифицированного профиля, не из произвольных env overrides.

```bash
./scripts/restore.sh /protected/backups/workspace-2026-10-08.tar.gz
```

Base проверяет профиль, endpoint, отдельное пустое назначение, целостность
архива и fingerprint ключа до восстановления. Wrapper не предоставляет
`--allow-source-project`, `--skip-file-volumes` или in-place `--clean`.
Отказ проверки не следует обходить ручным восстановлением в рабочие volumes.
После операции обязательны проверка данных, прав, readiness и продуктовых
сценариев; выполнение wrapper само по себе не доказывает готовность релиза.

## Старые архивы и автоматизация

Исторические архивы одного продукта (`dump` + attachments) сохраняются,
но не считаются совместимыми с форматом полного workspace. Для них нужна
отдельная согласованная процедура; этот wrapper не конвертирует их автоматически.
`cleanup_old_backups.sh` — историческая ротация, её нельзя применять к защищённым
workspace-архивам без отдельной политики хранения. Не подключайте прежний cron
с удалением копий к новому backup. WAL/PITR этой командой не настраивается.

## References

- [OPS_RUNBOOK](OPS_RUNBOOK.md)
- [STORAGE](STORAGE.md)
- [DEPLOYMENT](DEPLOYMENT.md)
