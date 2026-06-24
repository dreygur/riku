#!/bin/sh
# Riku one-line installer.
#
#   curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh
#
# Detects OS/arch, downloads the matching release binary, verifies its
# checksum, and installs it. Override with env vars:
#   RIKU_VERSION      release tag to install (default: latest)
#   RIKU_INSTALL_DIR  install directory (default: ~/.local/bin, or
#                     /usr/local/bin when running as root)
#   RIKU_NO_INIT=1    skip the post-install `riku init` hint
set -eu

REPO="dreygur/riku"
VERSION="${RIKU_VERSION:-latest}"

say()  { printf '%s\n' "$*"; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- detect platform ---------------------------------------------------------
os="$(uname -s)"
case "$os" in
  Linux)  os="linux" ;;
  Darwin) os="macos" ;;
  *)      err "unsupported OS '$os' (Linux and macOS only)" ;;
esac

arch="$(uname -m)"
case "$arch" in
  x86_64 | amd64)   arch="amd64" ;;
  aarch64 | arm64)  arch="arm64" ;;
  armv7l | armv7)   arch="armv7" ;;
  *)                err "unsupported architecture '$arch'" ;;
esac

# macOS ships only amd64/arm64 builds.
if [ "$os" = "macos" ] && [ "$arch" = "armv7" ]; then
  err "no macOS build for armv7"
fi

asset="riku-${os}-${arch}.tar.gz"

if [ "$VERSION" = "latest" ]; then
  base="https://github.com/${REPO}/releases/latest/download"
else
  base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

# --- pick a downloader -------------------------------------------------------
if have curl; then
  download() { curl -fsSL "$1" -o "$2"; }
elif have wget; then
  download() { wget -qO "$2" "$1"; }
else
  err "need curl or wget to download"
fi

# --- choose install dir ------------------------------------------------------
if [ -n "${RIKU_INSTALL_DIR:-}" ]; then
  install_dir="$RIKU_INSTALL_DIR"
elif [ "$(id -u)" = "0" ]; then
  install_dir="/usr/local/bin"
else
  install_dir="$HOME/.local/bin"
fi

# --- download + verify + install ---------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "-----> Downloading $asset ($VERSION)..."
download "${base}/${asset}" "${tmp}/${asset}" \
  || err "download failed: ${base}/${asset}"

if download "${base}/riku-${os}-${arch}.sha256" "${tmp}/sum" 2>/dev/null; then
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

say "-----> Installing to ${install_dir}/riku..."
tar -xzf "${tmp}/${asset}" -C "$tmp"
mkdir -p "$install_dir"
mv "${tmp}/riku" "${install_dir}/riku"
chmod +x "${install_dir}/riku"

say "✓ riku installed: ${install_dir}/riku"

# --- next steps --------------------------------------------------------------
case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *) say " !     ${install_dir} is not on your PATH. Add it:"
     say "         export PATH=\"${install_dir}:\$PATH\"" ;;
esac

if [ "${RIKU_NO_INIT:-0}" != "1" ]; then
  say ""
  say "Next: initialize the server"
  say "  ${install_dir}/riku init"
  say "Then scaffold and deploy your first app:"
  say "  ${install_dir}/riku quickstart"
fi
