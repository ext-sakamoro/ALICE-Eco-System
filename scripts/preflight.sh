#!/usr/bin/env bash
# scripts/preflight.sh — local reproduction of the CI gates before `git push`.
# Generated from the workflows below;
# every command is the one CI runs. A step this file does not cover is a step
# that can only fail remotely — when a workflow step is added, regenerate or
# add it here in the same commit. Run `--quick` before every push.
#
# usage: scripts/preflight.sh [--quick]   (--quick skips the test / bench suites)
set -euo pipefail
cd "$(dirname "$0")/.."

quick=0
[[ "${1:-}" == "--quick" ]] && quick=1

step() { printf '\n\033[1;34m== %s\033[0m\n' "$*"; }
# `cargo clippy` reuses fresh `cargo check` artifacts and then lints nothing;
# touching the crate roots invalidates only this repo's fingerprints.
relint() { git ls-files | grep -E '(^|/)src/(lib|main)\.rs$' | xargs -r touch; }
need() { command -v "$1" >/dev/null 2>&1 || { echo "missing tool: $1 ($2)" >&2; exit 1; }; }
has_toolchain() { rustup toolchain list | grep -q "^$1"; }

need actionlint "brew install actionlint"

step "ci.yml / fmt: Check formatting"
( export CARGO_TERM_COLOR="always"; cargo fmt -- --check )

step "ci.yml / actionlint: actionlint"
actionlint .github/workflows/*.yml

step "ci.yml / docs: Docs lint (tests)"
python3 scripts/test_docs_lint.py

step "ci.yml / docs: Docs lint (public documents, CHANGELOG structure)"
python3 scripts/docs_lint.py --check

step "ci.yml / docs: Crate index (README tables match docs/crate-index.tsv)"
python3 scripts/readme_index.py --check

if [[ $quick -eq 1 ]]; then
  echo; echo "preflight --quick OK (test / bench suites skipped)"; exit 0
fi

# needs network; CI runs it in crate-index.yml
step "crate-index.yml / online: Compare docs/crate-index.tsv with GitHub and crates.io"
python3 scripts/readme_index.py --online

echo; echo "preflight OK"
