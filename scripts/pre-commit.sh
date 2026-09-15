#!/bin/sh
# 提交前置门禁：与 .github/workflows/ci.yml 的检查项保持一致。
#
# 安装（克隆后执行一次，使 .git/hooks/pre-commit 指向本脚本）：
#   ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
#
# 跳过钩子（仅在明确知道后果时）：
#   git commit --no-verify
set -e
cd "$(git rev-parse --show-toplevel)"

echo "[pre-commit] Building web (rust-embed requires web/dist)"
cd web && pnpm build && cd ..

echo "[pre-commit] Running: cargo fmt --check"
cargo fmt --check

echo "[pre-commit] Running: clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "[pre-commit] Running: cargo test"
cargo test --all-targets

echo "[pre-commit] Running: web biome check"
cd web && pnpm lint && cd ..

echo "[pre-commit] Running: web vitest"
cd web && pnpm test run && cd ..
