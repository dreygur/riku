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
# Set them after sudo, which drops the caller's environment:
#   curl -fsSL .../install.sh | sudo RIKU_LIBC=musl sh
set -eu

REPO="dreygur/riku"
VERSION="${RIKU_VERSION:-latest}"
GLIBC_FLOOR_MAJOR=2
GLIBC_FLOOR_MINOR=31

say()  { printf '%s\n' "$*"; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

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
        printf 'glibc %s is older than %s.%s, installing the musl build\n' \
          "$found" "$GLIBC_FLOOR_MAJOR" "$GLIBC_FLOOR_MINOR" >&2
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

main() {
  bin="riku"
  while [ $# -gt 0 ]; do
    case "$1" in
      --rk) bin="rk" ;;
      *)    err "unknown argument '$1'" ;;
    esac
    shift
  done

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

  say "-----> Downloading $asset ($VERSION)..."
  download "${base}/${asset}" "${tmp}/${asset}" \
    || err "download failed: ${base}/${asset}"

  if download "${base}/${sum}" "${tmp}/sum" 2>/dev/null; then
    say "-----> Verifying checksum..."
    expected="$(awk '{print $1}' "${tmp}/sum")"
    if have sha256sum; then
      actual="$(sha256sum "${tmp}/${asset}" | awk '{print $1}')"
    elif have shasum; then
      actual="$(shasum -a 256 "${tmp}/${asset}" | awk '{print $1}')"
    else
      actual=""
      say " !     no sha256 tool found; skipping verification"
    fi
    if [ -n "$actual" ] && [ "$actual" != "$expected" ]; then
      err "checksum mismatch (expected $expected, got $actual)"
    fi
  else
    say " !     no checksum published for $asset; skipping verification"
  fi

  say "-----> Installing to ${install_dir}/${bin}..."
  tar -xzf "${tmp}/${asset}" --no-same-owner -C "$tmp"
  if [ "$(id -u)" = "0" ]; then
    chown 0:0 "${tmp}/${bin}"
  fi
  chmod 755 "${tmp}/${bin}"
  mkdir -p "$install_dir"
  mv "${tmp}/${bin}" "${install_dir}/${bin}"

  say "-----> ${bin} installed: ${install_dir}/${bin}"

  case ":${PATH}:" in
    *":${install_dir}:"*) ;;
    *) say " !     ${install_dir} is not on your PATH. Add it:"
       say "         export PATH=\"${install_dir}:\$PATH\"" ;;
  esac

  if [ "${RIKU_NO_INIT:-0}" != "1" ]; then
    say ""
    if [ "$bin" = "rk" ]; then
      say "Next: add a server"
      say "  ${install_dir}/rk server add myserver riku@edge.example --admin you@edge.example"
      say "It authorizes your ssh key on the server. Then run a command on it:"
      say "  ${install_dir}/rk -s myserver apps"
    else
      say "Next: initialize the server"
      say "  ${install_dir}/riku init"
    fi
  fi
}

# Tests source this file with RIKU_INSTALL_LIB=1 to call pick_platform alone.
[ -n "${RIKU_INSTALL_LIB:-}" ] || main "$@"
