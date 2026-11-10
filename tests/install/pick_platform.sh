#!/bin/sh
# Checks which release build scripts/install.sh picks for a host, with
# uname and getconf replaced by shell functions. Needs no network and no root.
#
#   sh tests/install/pick_platform.sh [path/to/install.sh]
#
# TEST_SH picks the shell the installer runs under (default sh), since users
# pipe it to sh, dash or bash.
set -u

test_sh="${TEST_SH:-sh}"

script="${1:-$(dirname "$0")/../../scripts/install.sh}"
failures=0
stderr_file="$(mktemp)"
trap 'rm -f "$stderr_file"' EXIT

# One case: kernel, machine, LONG_BIT, glibc version ("" = no glibc),
# RIKU_LIBC, expected stdout, and text expected on stderr ("" = none).
check() {
  # The inner script is single quoted on purpose: it expands in the child.
  # shellcheck disable=SC2016
  out="$(
    MOCK_OS="$1" MOCK_MACHINE="$2" MOCK_LONG_BIT="$3" MOCK_GLIBC="$4" \
      RIKU_LIBC="$5" RIKU_INSTALL_LIB=1 "$test_sh" -c '
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
    *"$7"*) ;;
    *)
      echo "FAIL $1 $2: stderr '$errors' lacks '$7'"
      failures=$((failures + 1))
      return
      ;;
  esac
  if [ -n "${8:-}" ]; then
    case "$errors" in
      *"$8"*)
        echo "FAIL $1 $2: stderr '$errors' should not say '$8'"
        failures=$((failures + 1))
        return
        ;;
    esac
  fi
  echo "ok   $1 $2 bits=$3 glibc=$4 libc=$5 -> $out"
}

check Linux armv6l  32 2.36 ""   linux-armv6      ""
check Linux aarch64 32 2.36 ""   linux-armv7      ""
check Linux aarch64 64 2.36 ""   linux-arm64      ""
check Linux x86_64  64 ""   ""   linux-amd64-musl ""
check Linux aarch64 64 2.28 ""   linux-arm64-musl "older than 2.31"
check Linux x86_64  64 2.39 musl linux-amd64-musl ""
check Darwin arm64  64 ""   musl macos-arm64      ""
check Linux x86_64  64 2.31 ""   linux-amd64      ""
check Linux aarch64 32 ""   ""   linux-armv7-musl ""

# Errors print nothing on stdout, so nothing gets downloaded.
check Linux x86_64  64 2.39 bogus "" "must be gnu or musl"
check Linux riscv64 64 2.39 ""   ""               "unsupported architecture"
check Linux x86_64  32 2.36 ""   ""               "no 32 bit x86 build"
check Linux aarch64 64 2.28 gnu  ""               "RIKU_LIBC=gnu" "installing the musl build"

[ "$failures" -eq 0 ] || { echo "$failures case(s) failed"; exit 1; }
