# 06: 质量工程对齐（web 子项目）

**What to build:** 把质量工程门禁扩展到 web 子项目，与后端同等。pre-commit hook 在现有 clippy + cargo test 之后追加 web 检查（Biome lint/format + vitest run）；GitHub CI（.github/workflows/ci.yml）增加 web job（安装 pnpm → `pnpm build` 产出 `web/dist`（rust-embed 编译期依赖）→ biome check → vitest run），并保证 rust job 构建前 `web/dist` 已存在。AGENTS.md 的 CI tracking 段同步。

**Blocked by:** 01, 04

**Status:** ready-for-agent

- [ ] pre-commit hook 增加 web 质量门，实测一次提交通过
- [ ] CI 含 web job + rust job 前置 web 构建，全部绿
- [ ] AGENTS.md CI tracking 段同步
- [ ] 本地 `cargo test/clippy/fmt` + `cd web && pnpm test/build/biome` 全绿