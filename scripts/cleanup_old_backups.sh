#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BACKUP_DIR="${PROJECT_DIR}/backups"
KEEP_DAYS=${1:-30}

find "$BACKUP_DIR" -maxdepth 1 -name 'task-tracker-*.tar.gz' -mtime +"$KEEP_DAYS" -delete

echo "Removed backups older than $KEEP_DAYS days."
