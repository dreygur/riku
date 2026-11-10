#!/bin/sh
# Checks which release build scripts/install.sh picks for a host, with
# uname and getconf replaced by shell functions. Needs no network and no root.
#
#   sh tests/install/pick_platform.sh [path/to/install.sh]
set -u

script="${1:-$(dirname "$0")/../../scripts/install.sh}"
failures=0
stderr_file="$(mktemp)"
trap 'rm -f "$stderr_file"' EXIT

# One case: kernel, machine, LONG_BIT, glibc version ("" = no glibc),
# RIKU_LIBC, expected stdout, and text expected on stderr ("" = none).
check() {
  out="$(
    MOCK_OS="$1" MOCK_MACHINE="$2" MOCK_LONG_BIT="$3" MOCK_GLIBC="$4" \
      RIKU_LIBC="$5" RIKU_INSTALL_LIB=1 sh -c '
        . "$0"
        uname() {
          case "$1" in
            -s) echo "$MOCK_OS" ;;
            -m) echo "$MOCK_MACHINE" ;;
          esac
        }
        getconf() {
          case "$1" in
            LONG_BIT) echo "$MOCK_LONG_BIT" ;;
            GNU_LIBC_VERSION)
              [ -n "$MOCK_GLIBC" ] || return 1
              echo "glibc $MOCK_GLIBC"
              ;;
          esac
        }
        pick_platform
      ' "$script" 2>"$stderr_file"
  )"
  errors="$(cat "$stderr_file")"
  if [ "$out" != "$6" ]; then
    echo "FAIL $1 $2 bits=$3 glibc=$4 libc=$5: got '$out', want '$6' ($errors)"
    failures=$((failures + 1))
    return
  fi
  case "$errors" in
    *"$7"*) echo "ok   $1 $2 bits=$3 glibc=$4 libc=$5 -> $out" ;;
    *)
      echo "FAIL $1 $2: stderr '$errors' lacks '$7'"
      failures=$((failures + 1))
      ;;
  esac
}

check Linux armv6l  32 2.36 ""   linux-armv6      ""
check Linux aarch64 32 2.36 ""   linux-armv7      ""
check Linux aarch64 64 2.36 ""   linux-arm64      ""
check Linux x86_64  64 ""   ""   linux-amd64-musl ""
check Linux aarch64 64 2.28 ""   linux-arm64-musl "older than 2.31"
check Linux x86_64  64 2.39 musl linux-amd64-musl ""
check Darwin arm64  64 ""   musl macos-arm64      ""

[ "$failures" -eq 0 ] || { echo "$failures case(s) failed"; exit 1; }
