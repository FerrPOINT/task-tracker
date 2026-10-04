#!/usr/bin/env bash
# Restore only to an explicitly prepared isolated, empty Compose destination.
set -euo pipefail
umask 077
: "${SDLC_TASK:?Set the maintenance owner task}"
: "${SDLC_WORKSPACE_DIR:?Select the workspace containing Base tools}"
: "${SDLC_RESTORE_PROJECT:?Select a unique sdlc-qa restore project}"
: "${SDLC_RESTORE_COMPOSE:?Select the isolated destination manifest}"
: "${SDLC_DOCKER_CONTEXT:?Select the Docker context}"
: "${SDLC_SIGNING_KEY:?Select the preserved Auth signing key}"
case "$SDLC_RESTORE_PROJECT" in sdlc-qa-*) ;; *) echo 'Restore requires an isolated QA project' >&2; exit 2 ;; esac
exec python3 "$SDLC_WORKSPACE_DIR/services-base/scripts/platform_backup.py" restore \
  --project "$SDLC_RESTORE_PROJECT" --compose-file "$SDLC_RESTORE_COMPOSE" \
  --project-directory "$SDLC_WORKSPACE_DIR" --docker-context "$SDLC_DOCKER_CONTEXT" \
  --task "$SDLC_TASK" --layout shared --signing-key-target "$SDLC_SIGNING_KEY" --archive "${1:?Specify a workspace backup archive}"
