# Установка CLI Task Tracker

Локальный candidate подготовлен 2026-10-02 из main `fd3337b200d1ec3ec6626900a9c96e2046cbed19`, Base `c083783a37791e277db796361203884b87828a7d`, Rust 1.88.0, `cargo build --locked --release -p task-tracker-cli`. Package version: `0.2.0`; версию продукта не меняли, Git tags и публичный release не создавали.

## Артефакт и требования

Архив: `task-tracker-cli-0.2.0-fd3337b-x86_64-linux-gnu.tar.gz`. SHA-256 архива: `6e50cf47da5bfa282f4ff3c542cdbbaaebe1b4a5a65e387028b9a9d791bffb2d`. Внутри только `task-tracker`, `README.md` и `SHA256SUMS`; credentials и данные стенда не включены.

GNU/Linux x86_64, glibc >= 2.34, OpenSSL 3 (libssl.so.3 и libcrypto.so.3). Живой API-прогон выполнен в Ubuntu 24.04 WSL; запуск/справка дополнительно проверены в Debian 12. Native Windows/macOS и musl-сборки не подготовлены. На Windows используйте WSL. Версию можно прочитать через `task-tracker --version`.

## Установка в Linux / WSL

После получения архива сверьте его SHA-256 с manifest, затем:

```bash
workdir=$(mktemp -d)
tar -xzf task-tracker-cli-0.2.0-fd3337b-x86_64-linux-gnu.tar.gz -C "$workdir"
(cd "$workdir" && sha256sum -c SHA256SUMS)
mkdir -p "$HOME/.local/bin"
install -m 755 "$workdir/task-tracker" "$HOME/.local/bin/task-tracker"
export PATH="$HOME/.local/bin:$PATH"
task-tracker --help
```

При существующей установке сначала сохраните предыдущий binary для отката. Для проверки этой поставки использован отдельный prefix `/root/.local/share/sdlc-cli/cli-20261002/bin`; прежние команды не перезаписывались.

## Подключение к sdlc1

```bash
export TASKTRACKER_API_URL=http://127.0.0.1:7721/api/v1
task-tracker project list
```

Передайте PAT через `SDLC_API_TOKEN` либо продуктовую переменную из [CLI.md](CLI.md). Read-команды требуют `task-tracker:read`, mutations — соответствующий `task-tracker:write`; scopes не отменяют серверную авторизацию. Значение token не помещайте в аргументы, историю shell, manifest или release notes. URL здесь относится к sdlc1; для другого стенда задайте его явно.

## Статус приёмки и откат

Это candidate: полная приёмка против принятого runtime не завершена. Ограничения и результаты — [CLI_VALIDATION.md](CLI_VALIDATION.md). Обновление CLI не обновляет backend; рабочие runtime images/pins в этой проверке не заменялись. После согласованного обновления backend повторить блокирующие сценарии; затем решать о tags, версиях и публикации release. Для отката верните предыдущий binary и конфигурацию URL.

## References

- [CLI](CLI.md)
- [CLI validation](CLI_VALIDATION.md)
- [Base integration](BASE_INTEGRATION.md)

## Новый локальный candidate с текущим Base pin

Source `a1f67ce088bb6f2c0b1046de0c17bc1194f48c9d`; Base `9408802dfa978cba2f67162a49adca6f65851b01`, Rust 1.88.0, locked release workspace build, package version `0.2.0` без изменения. Архив `task-tracker-cli-0.2.0-a1f67ce-x86_64-linux-gnu.tar.gz`; SHA-256 `0321131c0f62af1cdd7d2a8ca6f53a263e6ff984c972a168eb053a71783317df`. Требования: glibc >= 2.34; OpenSSL 3 (libssl.so.3/libcrypto.so.3). Установка с SHA256SUMS в отдельный prefix и запуск --help проверены в Ubuntu 24.04 WSL и Debian 12. Прежние binaries сохранены; shell profiles не менялись.

Результаты QA и blockers сохранены в [CLI_VALIDATION.md](CLI_VALIDATION.md). Это candidate: рабочий sdlc1 не обновлён, совместимость CI/CD с применёнными миграциями 36/37 и прежний image rollback не подтверждены. Архивы не содержат configuration, keys, credentials или данные.
