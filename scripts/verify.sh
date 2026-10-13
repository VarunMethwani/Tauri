#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ ! -f desktop/package-lock.json ]]; then
  printf '%s\n' 'Missing desktop/package-lock.json. Resolve dependencies and review the lockfile before running this gate.' >&2
  exit 1
fi

npm --prefix desktop ci
npm --prefix desktop audit
npm --prefix desktop audit --omit=dev
npm --prefix desktop run build
cargo fmt --all --check
cargo test --workspace --exclude harness-desktop
cargo check -p harness-desktop

printf '%s\n' 'Local install, audits, frontend build, Rust format/tests, and native compile checks passed.'
