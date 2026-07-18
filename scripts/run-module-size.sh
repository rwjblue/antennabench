#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

mkdir -p target/module-size
{
  node --test scripts/validate-module-size.test.mjs
  node scripts/validate-module-size.mjs
} 2>&1 | tee target/module-size/last-run.log
