#!/usr/bin/env bash
set -euo pipefail
cargo install cargo-tarpaulin --locked 2>/dev/null || true
cargo tarpaulin \
  --out Html \
  --out Lcov \
  --output-dir coverage/ \
  --exclude-files 'tests/*' \
  --exclude-files 'src/*/tests.rs' \
  --ignore-tests \
  --timeout 300
echo "Coverage report: coverage/tarpaulin-report.html"
