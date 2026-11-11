#!/bin/sh
# Riku one-line installer.
#
#   curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sudo sh
#   curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh -s -- --rk
#
# Detects OS, CPU and C library, downloads the matching release binary,
# verifies its checksum, and installs it. The default installs riku; --rk
# installs the rk laptop client instead. Override with env vars:
#   RIKU_VERSION      release tag to install (default: latest)
#   RIKU_INSTALL_DIR  install directory (default: /usr/local/bin as root, or
#                     ~/.local/bin for rk; riku itself installs only as root)
#   RIKU_LIBC         gnu or musl, on Linux only (default: gnu when glibc
#                     2.31 or newer is present, otherwise the static musl build)
#   RIKU_NO_INIT=1    skip the post-install next-step hint
#   RIKU_SKIP_CHECKSUM=1  install even when the release's .sha256 cannot be
#                     fetched or no sha256 tool exists (default: stop). A
#                     checksum that does not match always stops.
# Set them after sudo, which drops the caller's environment:
#   curl -fsSL .../install.sh | sudo RIKU_LIBC=musl sh
set -eu

REPO="dreygur/riku"
VERSION="${RIKU_VERSION:-latest}"
GLIBC_FLOOR_MAJOR=2
GLIBC_FLOOR_MINOR=31

say()  { printf '%s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# Output follows riku's rail: an intro, one line per step, warnings and
# errors on stderr, an outro and a summary block. Colors only reach a
# terminal, and never when NO_COLOR is set to something. Both streams are
# tested here, once: inside a command substitution stdout is a pipe.
colour_1=""
colour_2=""
if [ -z "${NO_COLOR:-}" ]; then
  if [ -t 1 ]; then colour_1=1; fi
  if [ -t 2 ]; then colour_2=1; fi
fi

# paint FD CODE TEXT: TEXT in ANSI style CODE when FD is a terminal.
paint() {
  if { [ "$1" = 1 ] && [ -n "$colour_1" ]; } || { [ "$1" = 2 ] && [ -n "$colour_2" ]; }; then
    printf '\033[%sm%s\033[0m' "$2" "$3"
  else
    printf '%s' "$3"
  fi
}

# line FD CODE GLYPH [LABEL [DETAIL]]: one rail line, DETAIL dimmed.
line() {
  text="$(paint "$1" "$2" "$3")"
  if [ -n "${4:-}" ]; then
    text="$text  $4"
  fi
  if [ -n "${5:-}" ]; then
    text="$text  $(paint "$1" 2 "$5")"
  fi
  printf '%s\n' "$text" >&"$1"
}

rail()  { line 1 90 "│" "${1:-}"; }
intro() { line 1 90 "┌" "$(paint 1 1 "$1")" "$2"; rail; }
step()  { line 1 32 "◇" "$1" "${2:-}"; }
warn()  { line 2 33 "▲" "$1"; }
err()   { line 2 31 "■" "$(paint 2 31 "$*")"; exit 1; }
outro() { rail; line 1 90 "└" "$(paint 1 1 "$1")"; }

# summary_row KEY VALUE: one line of the block after the outro. Every key
# this script prints is four characters, so the values line up.
summary_row() { printf '   %s   %s\n' "$(paint 1 1 "$1")" "$2"; }

# The CPU name used in release assets. A 64 bit kernel can run a 32 bit
# userland (common on Raspberry Pi OS); that userland has no arm64 libraries,
# so it gets the armv7 build, which the CPU runs.
linux_arch() {
  machine="$(uname -m)"
  case "$machine" in
    x86_64 | amd64)
      if [ "$(getconf LONG_BIT 2>/dev/null || echo 64)" = "32" ]; then
        err "no 32 bit x86 build; this kernel is 64 bit but the userland is 32 bit"
      fi
      say "amd64"
      ;;
    aarch64 | arm64)
      if [ "$(getconf LONG_BIT 2>/dev/null || echo 64)" = "32" ]; then
        say "armv7"
      else
        say "arm64"
      fi
      ;;
    armv7l | armv7 | armv8l) say "armv7" ;;
    armv6l | armv6) say "armv6" ;;
    *) err "unsupported architecture '$machine'" ;;
  esac
}

# The glibc version, such as 2.36, or "" when there is none. `getconf
# GNU_LIBC_VERSION` prints "glibc 2.36" and fails on musl systems such as
# Alpine.
glibc_version() {
  version="$(getconf GNU_LIBC_VERSION 2>/dev/null || true)"
  say "${version##* }"
}

# Succeeds when glibc 2.31 or newer is present.
glibc_usable() {
  version="$(glibc_version)"
  major="${version%%.*}"
  minor="${version#*.}"
  minor="${minor%%.*}"
  case "$major$minor" in
    '' | *[!0-9]*) return 1 ;;
  esac
  if [ "$major" -ne "$GLIBC_FLOOR_MAJOR" ]; then
    [ "$major" -gt "$GLIBC_FLOOR_MAJOR" ]
    return
  fi
  [ "$minor" -ge "$GLIBC_FLOOR_MINOR" ]
}

# "" for the glibc build, "-musl" for the static one.
libc_suffix() {
  case "${RIKU_LIBC:-}" in
    musl) say "-musl" ;;
    gnu)
      glibc_usable || err "RIKU_LIBC=gnu, but no glibc ${GLIBC_FLOOR_MAJOR}.${GLIBC_FLOOR_MINOR} or newer was found"
      say ""
      ;;
    '')
      if glibc_usable; then
        say ""
        return
      fi
      found="$(glibc_version)"
      if [ -n "$found" ]; then
        warn "glibc $found is older than ${GLIBC_FLOOR_MAJOR}.${GLIBC_FLOOR_MINOR}, installing the musl build"
      fi
      say "-musl"
      ;;
    *) err "RIKU_LIBC must be gnu or musl, not '${RIKU_LIBC}'" ;;
  esac
}

# The release platform, such as linux-armv6 or linux-arm64-musl. Notices go to
# stderr so stdout stays the platform alone.
pick_platform() {
  os="$(uname -s)"
  case "$os" in
    Linux)
      arch="$(linux_arch)"
      suffix="$(libc_suffix)"
      say "linux-${arch}${suffix}"
      ;;
    Darwin)
      case "$(uname -m)" in
        x86_64) say "macos-amd64" ;;
        arm64)  say "macos-arm64" ;;
        *)      err "no macOS build for '$(uname -m)'" ;;
      esac
      ;;
    *) err "unsupported OS '$os' (Linux and macOS only)" ;;
  esac
}

# unverified REASON: stop with REASON, or with RIKU_SKIP_CHECKSUM=1 warn
# and go on without a checksum.
unverified() {
  if [ "${RIKU_SKIP_CHECKSUM:-0}" = "1" ]; then
    warn "$1; RIKU_SKIP_CHECKSUM=1, installing unverified"
    return 0
  fi
  err "$1 (set RIKU_SKIP_CHECKSUM=1 to install unverified)"
}

# verify_checksum FILE URL SUMFILE: compare FILE's sha256 with the one
# published at URL, fetched into SUMFILE. Fails closed: the binary is
# installed as root, so a checksum that cannot be fetched or computed stops
# the install unless RIKU_SKIP_CHECKSUM=1.
verify_checksum() {
  if ! download "$2" "$3" 2>/dev/null; then
    unverified "could not fetch the checksum $2"
    return 0
  fi
  expected="$(awk '{print $1}' "$3")"
  if have sha256sum; then
    actual="$(sha256sum "$1" | awk '{print $1}')"
  elif have shasum; then
    actual="$(shasum -a 256 "$1" | awk '{print $1}')"
  else
    unverified "no sha256sum or shasum to check the download with"
    return 0
  fi
  if [ "$actual" != "$expected" ]; then
    err "checksum mismatch (expected $expected, got $actual)"
  fi
  step "Checksum" "sha256 matches"
}

main() {
  bin="riku"
  while [ $# -gt 0 ]; do
    case "$1" in
      --rk) bin="rk" ;;
      *)    err "unknown argument '$1'" ;;
    esac
    shift
  done

  intro "${bin} install" "$VERSION"
  platform="$(pick_platform)"
  asset="${bin}-${platform}.tar.gz"
  sum="${bin}-${platform}.sha256"

  if [ "$VERSION" = "latest" ]; then
    base="https://github.com/${REPO}/releases/latest/download"
  else
    base="https://github.com/${REPO}/releases/download/${VERSION}"
  fi

  if have curl; then
    download() { curl -fsSL "$1" -o "$2"; }
  elif have wget; then
    download() { wget -qO "$2" "$1"; }
  else
    err "need curl or wget to download"
  fi

  if [ "$bin" = "riku" ] && [ "$(id -u)" != "0" ]; then
    err "riku installs to /usr/local/bin as root, run this script with sudo (rk needs no root)"
  fi

  if [ -n "${RIKU_INSTALL_DIR:-}" ]; then
    install_dir="$RIKU_INSTALL_DIR"
  elif [ "$(id -u)" = "0" ]; then
    install_dir="/usr/local/bin"
  else
    install_dir="$HOME/.local/bin"
  fi

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  download "${base}/${asset}" "${tmp}/${asset}" \
    || err "download failed: ${base}/${asset}"
  step "Download" "$asset"

  verify_checksum "${tmp}/${asset}" "${base}/${sum}" "${tmp}/sum"

  tar -xzf "${tmp}/${asset}" --no-same-owner -C "$tmp"
  if [ "$(id -u)" = "0" ]; then
    chown 0:0 "${tmp}/${bin}"
  fi
  chmod 755 "${tmp}/${bin}"
  mkdir -p "$install_dir"
  mv "${tmp}/${bin}" "${install_dir}/${bin}"
  step "Install" "${install_dir}/${bin}"

  case ":${PATH}:" in
    *":${install_dir}:"*) ;;
    *) warn "${install_dir} is not on your PATH. Add it:"
       rail "export PATH=\"${install_dir}:\$PATH\"" ;;
  esac

  outro "${bin} installed"

  if [ "${RIKU_NO_INIT:-0}" != "1" ]; then
    say ""
    if [ "$bin" = "rk" ]; then
      summary_row "Next" "${install_dir}/rk server add myserver riku@edge.example --admin you@edge.example"
      summary_row "    " "(authorizes your ssh key on the server)"
      summary_row "Then" "${install_dir}/rk -s myserver apps"
    else
      summary_row "Next" "${install_dir}/riku init"
    fi
  fi
}

# Tests source this file with RIKU_INSTALL_LIB=1 to call pick_platform alone.
[ -n "${RIKU_INSTALL_LIB:-}" ] || main "$@"
