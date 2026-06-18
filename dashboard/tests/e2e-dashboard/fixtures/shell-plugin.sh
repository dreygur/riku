#!/usr/bin/env bash
# Riku runtime plugin: shell
# Matches apps that set RUNTIME=shell explicitly (see detect: always exit 1
# so it is never auto-detected; this plugin only exists to satisfy the
# E2E stress suite's fixture app, which is a bare shell script with no
# package.json/requirements.txt/etc. for the bundled plugins to key off).
set -euo pipefail

CMD="${1:-}"
APP_PATH="${RIKU_APP_PATH:-$(pwd)}"

case "$CMD" in
  detect)
    exit 1
    ;;

  build)
    exit 0
    ;;

  env)
    exit 0
    ;;

  start)
    echo "$APP_PATH/web.sh"
    ;;

  *)
    echo "Unknown subcommand: $CMD" >&2
    exit 1
    ;;
esac
