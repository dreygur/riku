# Small Boards and Binary Size Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship riku and rk for every Raspberry Pi (armv6, armv7, arm64) as glibc and static musl builds that run on current Debian based systems, and measure sizes so the owner can set a budget.

**Architecture:** The release workflow builds Linux targets with cargo-zigbuild, which pins a glibc floor and handles musl and armv6 without per target linkers. riku learns its own target triple at build time, so plugin downloads match the binary that asks. The installer maps `armv6l`, handles a 32 bit userland on a 64 bit kernel, and picks musl when the host has no usable glibc.

**Tech Stack:** GitHub Actions, cargo-zigbuild with zig, POSIX sh, Docker with qemu for smoke runs.

**Spec:** `docs/superpowers/ROADMAP.md`, section "After developer experience: small boards and binary size". There is no separate design; the roadmap item is the spec. EGM readiness and codebase reviews ran on 2026-11-10 and their findings are applied below.

## Findings that shaped this plan (verified 2026-11-10 on the v0.1.3 assets)

- **Bug, P1:** every Linux release binary needs `GLIBC_2.39` because it is built on `ubuntu-latest` (24.04). Run under Docker: v0.1.3 arm64 riku in `debian:bookworm` fails with ``version `GLIBC_2.39' not found``; in `ubuntu:24.04` it prints `riku 0.1.3`. So riku does not start on Debian 12, Raspberry Pi OS bookworm or Ubuntu 22.04.
- **Bug, same cause:** `.github/workflows/ci.yml:149-177` has a second `release` job that builds x86_64 gnu on `ubuntu-latest` and uploads `riku-<tag>.tar.gz` to the same release. Nothing in the repo downloads that asset.
- **Bug:** `crates/riku-cli/src/apps/plugin_download.rs:77` maps `(linux, arm)` to `armv7-unknown-linux-gnueabihf` (function `target_triple_for` at :72-84, called at :131). An armv6 riku would fetch an armv7 plugin that dies with an illegal instruction on a Pi Zero; a musl riku would fetch a glibc plugin.
- **Gap:** `scripts/install.sh:40-46` has no `armv6l` case. The checksum URL at :91 is built from `${os}-${arch}`, not from `$asset`.
- **Profile:** `Cargo.toml:58-62` already has `opt-level = "z"`, `lto = true`, `strip = true`, `codegen-units = 1`. `Cargo.toml:54-57` records why riku keeps `panic = "unwind"` (the supervisor survives thread and task panics, e.g. `riku-container/src/engine/probe.rs:52`, `riku-supervisor/src/health/control.rs:109`). Only rk is a candidate for `abort`.
- **v0.1.3 sizes**, stripped (gzip in brackets):

  | target | riku | rk |
  | --- | --- | --- |
  | linux-amd64 | 9.58 MB (4.29) | 1.35 MB (0.64) |
  | linux-arm64 | 8.14 MB (4.15) | 1.18 MB (0.64) |
  | linux-armv7 | 7.76 MB (3.81) | 1.24 MB (0.64) |

## Global Constraints

- Confidential names: never write the owner's server, organisation or project names. Search the diff before every commit.
- No `?` in new or touched Rust; tests in sibling `_tests.rs` files via `#[path]`; doc comments on public items; no emoji or em dashes.
- No python, perl or other interpreters to read or edit files; native file tools only.
- CODEMAP and ROADMAP updated in the same commit as the code they describe.
- glibc floor **2.31** (Debian 11, Ubuntu 20.04, Raspberry Pi OS bullseye). My call; the owner may move it.
- macOS builds are out of scope and stay as they are.
- Asset names: glibc keeps `<bin>-linux-<arch>.tar.gz` and `.sha256`; musl adds `-musl` (`riku-linux-arm64-musl.tar.gz`). Plugin assets keep the plain Rust triple (`riku-plugin-java-aarch64-unknown-linux-musl`).
- The eight Linux triples (`matrix.target` is always the plain triple):

  | Rust target | platform | board |
  | --- | --- | --- |
  | x86_64-unknown-linux-gnu | linux-amd64 | PC, VPS |
  | x86_64-unknown-linux-musl | linux-amd64-musl | PC, VPS, Alpine |
  | aarch64-unknown-linux-gnu | linux-arm64 | Pi 3/4/5, Zero 2 W, 64 bit OS |
  | aarch64-unknown-linux-musl | linux-arm64-musl | same, no glibc |
  | armv7-unknown-linux-gnueabihf | linux-armv7 | Pi 2/3/4/5, 32 bit OS |
  | armv7-unknown-linux-musleabihf | linux-armv7-musl | same, no glibc |
  | arm-unknown-linux-gnueabihf | linux-armv6 | Pi 1, Zero, Zero W |
  | arm-unknown-linux-musleabihf | linux-armv6-musl | same, no glibc |

## Review Focus

1. A 64 bit kernel with a 32 bit userland (Pi 3/4 on 32 bit Raspberry Pi OS): `uname -m` says `aarch64`, `getconf LONG_BIT` says 32. Expected: `linux-armv7`, which runs on that CPU and userland. Test in Task 3.
2. Alpine or another musl host: expected the musl build, and that riku fetches musl plugins. Tests in Tasks 2 and 3.
3. glibc older than 2.31: expected the musl build plus a stderr line naming the floor, not a loader error. Test in Task 3.
4. A riku built for a triple the release does not ship: `install-plugins java` errors naming the plugin and the triple. Test in Task 2.
5. armv6 binaries must not contain armv7 instructions: the smoke run pins `QEMU_CPU=arm1176`. Task 1.

---

### Task 1: Release builds for all Pi targets with a glibc floor

**Files:**
- Modify: `.github/workflows/release.yml`, `.github/workflows/ci.yml:149-177` (delete the `release` job)
- Modify: `docs/CODEMAP.md` (release section), `docs/superpowers/ROADMAP.md`, `CHANGELOG.md` (Unreleased)

- [ ] **Step 1: Prerequisites.** `brew install zig`, `cargo install cargo-zigbuild`, `rustup target add` for all eight triples. Docker Desktop registers qemu binfmt; on plain Docker run `docker run --privileged --rm tonistiigi/binfmt --install arm,arm64` first.
- [ ] **Step 2: Rewrite the Linux build rows** of `release.yml`:
  - install zig and cargo-zigbuild in the job (pin versions and action SHAs, as the file does now);
  - add a `glibc` matrix field, `'.2.31'` for gnu rows and `''` for musl rows, and build with `cargo zigbuild --release --target ${{ matrix.target }}${{ matrix.glibc }}`. Output still lands in `target/<plain triple>/release`, so the artifact paths, the cache key and the plugin bundle name keep using `matrix.target` unchanged;
  - set `platform` from the table above; add the five new rows (armv6 gnu plus four musl);
  - delete the two `.cargo/config.toml` linker steps, the apt cross gcc installs and the strip step (`strip = true` in the profile already applies);
  - on `softprops/action-gh-release` set `prerelease: ${{ contains(github.ref_name, '-') }}`, so an rc tag never becomes `latest`.
- [ ] **Step 3: Delete the `release` job in `ci.yml`.** It duplicates release.yml with the glibc 2.39 bug and its asset has no consumer (`git grep` for `riku-v` and `riku-${` finds none outside it).
- [ ] **Step 4: Build all eight triples locally** with the same commands as the workflow, for both `riku` and `rk`. Expected: all build. For each gnu build, `strings <bin> | grep -o 'GLIBC_2\.[0-9]*' | sort -uV | tail -1` prints `GLIBC_2.31` or lower.
- [ ] **Step 5: Smoke run** `riku --version` and `rk --version` (binaries mounted read only, `--rm`). Expected: each prints the workspace version.
  - amd64 gnu in `debian:bullseye` (`--platform linux/amd64`), amd64 musl in `alpine:3.20`;
  - arm64 gnu in `debian:bullseye` (`--platform linux/arm64`), arm64 musl in `alpine:3.20`;
  - armv7 gnu in `debian:bullseye` (`--platform linux/arm/v7`), armv7 musl in `alpine:3.20` (`linux/arm/v7`);
  - armv6 musl in `alpine:3.20` with `--platform linux/arm/v6 -e QEMU_CPU=arm1176`;
  - armv6 gnu in `balenalib/raspberry-pi-debian:bullseye` (Raspbian armhf, armv6) with `--platform linux/arm/v6 -e QEMU_CPU=arm1176`. Debian's own armhf needs v7, so it cannot prove armv6.
- [ ] **Step 6: Commit** "Build Pi targets with a glibc 2.31 floor". The commit body and the ROADMAP status line say the workflow is proven only by the next tag.

### Task 2: Plugin downloads use riku's own target

**Files:**
- Create: `crates/riku-cli/build.rs`
- Modify: `crates/riku-cli/src/apps/plugin_download.rs:72-84` and its call at `:131`, `crates/riku-cli/src/apps/plugin_download_tests.rs` (delete `host_target_names_the_plugin_when_unsupported` at :41-51, which tests the removed function), `docs/CODEMAP.md`

**Interfaces:**
- `build.rs`: `match std::env::var("TARGET")`, printing `cargo:rustc-env=RIKU_TARGET=<value>`; panic with a message if unset (cargo always sets it).
- Produces: `fn release_triple(name: &str, target: &str) -> anyhow::Result<&'static str>`. Looks `target` up in a `const RELEASE_TRIPLES: &[&str]` (the eight Linux triples plus `x86_64-apple-darwin` and `aarch64-apple-darwin`) and returns the matching `&'static str` from the slice. Otherwise the error names the plugin and the triple, e.g. `no java build for riscv64gc-unknown-linux-gnu`. The caller at `:131` passes `env!("RIKU_TARGET")`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn armv6_riku_fetches_the_armv6_plugin() {
    assert_eq!(release_triple("java", "arm-unknown-linux-gnueabihf").unwrap(), "arm-unknown-linux-gnueabihf");
}

#[test]
fn musl_riku_fetches_the_musl_plugin() {
    assert_eq!(release_triple("java", "aarch64-unknown-linux-musl").unwrap(), "aarch64-unknown-linux-musl");
}

#[test]
fn unshipped_target_names_plugin_and_triple() {
    let err = release_triple("java", "riscv64gc-unknown-linux-gnu").unwrap_err().to_string();
    assert!(err.contains("java") && err.contains("riscv64gc-unknown-linux-gnu"), "{err}");
}
```

- [ ] **Step 2: Run** `cargo test -p riku-cli plugin_download`. Expected: FAIL, `release_triple` not found.
- [ ] **Step 3: Implement** `release_triple` and `build.rs`; remove `target_triple_for` and its test.
- [ ] **Step 4: Run** `cargo test -p riku-cli plugin_download`. Expected: PASS.
- [ ] **Step 5: Commit** "Fetch plugins built for riku's own target".

### Task 3: Installer for armv6, 32 bit userland, musl and old glibc

**Files:**
- Modify: `scripts/install.sh` (whole flow: :14-116)
- Create: `tests/install/pick_platform.sh`
- Modify: `.github/workflows/ci.yml` (one step in the existing `ci` job: `sh tests/install/pick_platform.sh`)
- Docs: `docs/docs/installation.md:9-10` and a platform table near :78-84, `README.md:26,68`, `scripts/README.md:5`, `docs/docs/container-images.md:16-17` if it contradicts the table, `docs/CODEMAP.md`

**Interfaces:**
- Restructure: helpers at the top, everything after them in `main()`, and the last line `[ -n "${RIKU_INSTALL_LIB:-}" ] || main "$@"`. The test sets `RIKU_INSTALL_LIB=1` and sources the script, so nothing downloads, parses arguments or checks root.
- Produces: `pick_platform` printing the platform from the table (`linux-armv6`, `linux-arm64-musl`, `macos-arm64`) on stdout; any notice goes to stderr. `main` builds both `${bin}-${platform}.tar.gz` and `${bin}-${platform}.sha256` from it (fixes :91).
- Inputs, each overridable for the test: `uname -s`, `uname -m`, `getconf LONG_BIT`, `getconf GNU_LIBC_VERSION` (prints `glibc 2.36`; take the last field, compare major and minor numerically against 2.31; a failed call means no glibc), and `RIKU_LIBC` (`gnu` or `musl`, overrides detection on Linux). The test overrides commands by defining shell functions with the same names after sourcing.
- Rules: Darwin skips libc detection and ignores `RIKU_LIBC`. `aarch64` with `LONG_BIT` 32 is treated as `armv7`. `armv6l` is `armv6`. No glibc, or glibc below 2.31, gives the `-musl` platform; below 2.31 also prints `glibc <v> is older than 2.31, installing the musl build` to stderr. `RIKU_LIBC=gnu` with no usable glibc is an error.

- [ ] **Step 1: Write the failing test** `tests/install/pick_platform.sh`, one case per line, asserting stdout:
  - `armv6l`, glibc 2.36 gives `linux-armv6`;
  - `aarch64`, `LONG_BIT` 32, glibc 2.36 gives `linux-armv7`;
  - `x86_64`, `getconf GNU_LIBC_VERSION` failing (Alpine) gives `linux-amd64-musl`;
  - `aarch64`, glibc 2.28 gives `linux-arm64-musl` and the floor line on stderr;
  - `x86_64`, glibc 2.39, `RIKU_LIBC=musl` gives `linux-amd64-musl`;
  - `Darwin` `arm64` with `RIKU_LIBC=musl` gives `macos-arm64`.
- [ ] **Step 2: Run** `sh tests/install/pick_platform.sh`. Expected: FAIL (`pick_platform` not found).
- [ ] **Step 3: Implement** the restructure and `pick_platform`; update the docs listed above.
- [ ] **Step 4: Run** the test and `shellcheck scripts/install.sh tests/install/pick_platform.sh`. Expected: PASS, no warnings.
- [ ] **Step 5: Commit** "Install the right build on Pi and musl hosts".

### Task 4: Size measurement

**Files:**
- Modify: `Cargo.toml` (after `[profile.release]`: `[profile.release-abort]` with `inherits = "release"` and `panic = "abort"`; comment that it exists only to measure rk)
- Modify: `docs/superpowers/ROADMAP.md`

- [ ] **Step 1: Measure**, from the Task 1 builds, riku and rk for all eight triples (stripped bytes and gzip bytes): 16 rows. Then build rk only with `--profile release-abort` for the eight triples (output in `target/<triple>/release-abort`): 8 more numbers.
- [ ] **Step 2: Record** both tables in the ROADMAP small boards section, with the v0.1.3 numbers above for comparison. riku stays on `unwind` (Cargo.toml:54-57). If `abort` saves rk more than 10 percent, propose it for rk as the owner's call; otherwise remove `release-abort`.
- [ ] **Step 3: Commit** "Record binary sizes per Pi target".

### Final

- Full suite, clippy, fmt; one whole branch review on Opus; one fix round.
- Ask the owner before pushing, opening the PR or tagging. The release workflow is proven only by a tag. With `prerelease` set, a `v0.1.4-rc.1` tag is safe for `latest`, but `verify-version` refuses it unless the workspace version is `0.1.4-rc.1`; the owner decides on that bump.
