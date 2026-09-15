# AGENTS.md

## 项目概述

通用单用户自托管后台模板：Rust（axum + SeaORM + SQLite）单体 + React 管理后台，前端产物由 `rust-embed` 内嵌进二进制。内置登录会话、设置（语言/时区）、定时任务引擎（含执行日志 SSE）、双语 i18n 与示例域 Notes。新项目以此起步，把 Notes 换成自己的业务域即可。

决策记录见 `docs/adr/`，域词汇见 `CONTEXT.md`，来源与边界见 `docs/adr/0001-template-from-llm-gateway.md`。

## 技术栈

**后端**：axum 0.8、SeaORM 2（sqlite）、tokio、tower-http（CORS/trace/catch-panic）、argon2id、aes-gcm、croner（cron 表达式）、tracing（结构化 JSON 日志）、rust-embed。

**前端**：React 19 + TypeScript（严格模式）、Vite、Tailwind CSS、shadcn/ui（Radix）、TanStack Query、Zustand、react-i18next、ky、zod。

## 项目结构

```
src/
├── main.rs              # 入口
├── lib.rs               # init/run、tracing 装配、优雅关停
├── config/              # 环境变量解析与校验
├── db.rs                # 连接配置（WAL）+ 迁移
├── entity/              # SeaORM 实体（user/session/setting/cron_job*/note）
├── auth/                # 密码哈希、会话、鉴权中间件
├── crypto/              # SECRET_KEY 加解密与掩码
├── app_settings.rs      # 语言/时区进程内缓存
├── i18n.rs              # 后端双语消息
├── response.rs          # 统一响应信封与错误 helper
├── middleware/          # CORS、panic 处理等
├── static_assets/       # rust-embed 静态资源与 SPA fallback
├── logs_cleanup.rs      # 日志目录定期清理
├── cron/                # 定时任务引擎（parser/repository/scheduler/worker/log*）
└── routes/              # HTTP 路由（auth/cron_jobs/notes/settings）
web/src/
├── main.tsx  App.tsx    # 入口与路由
├── pages/               # 页面
├── components/          # 业务组件与 ui/ 原语
├── hooks/               # 数据与状态 hooks
├── lib/                 # api 封装、工具函数、常量
├── i18n/                # i18next 初始化与 locales
└── test/setup.ts        # vitest 全局桩
tests/                   # HTTP 集成测试（common/ 为共享引导）
```

约定：单文件不超过 1000 行；超限时按域拆子模块（`mod.rs` + 子文件，或 `tests/<stem>/` 由根文件 `#[path]` 引入）。

## 构建与运行

### 后端

```bash
cargo build             # 调试构建
cargo build --release   # 发布构建（内嵌 web/dist）
cargo run               # 需要环境变量，见 .env.example
```

### 前端

```bash
cd web
pnpm install
pnpm dev                # 开发服务器，代理 /api 到后端
pnpm build              # tsc -b + vite build，产物到 web/dist
pnpm lint               # biome check .
pnpm test run           # vitest 一次性运行（pnpm test 为 watch）
```

### 完整本地启动

```bash
cd web && pnpm build && cd ..          # 后端编译期内嵌 web/dist，必须先构建前端
export BIND_ADDRESS=127.0.0.1:4007 APP_ENV=dev DATABASE_URL='sqlite://db/app.db?mode=rwc'
cargo run
```

### 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `BIND_ADDRESS` | `0.0.0.0:4007` | 监听地址 |
| `APP_ENV` | `dev` | `dev`/`prod`；非法值静默回退 `dev` |
| `DATABASE_URL` | dev 为 `sqlite://db/app.db?mode=rwc` | SQLite 连接串，父目录会自动创建 |
| `RUST_LOG` | `info,sqlx::query=warn` | 日志过滤 |
| `CRON_JOB_QUEUE_SIZE` | `1000` | 任务队列容量（须为正整数） |
| `CRON_JOB_MAX_CONCURRENT` | `10` | 并发执行上限（须为正整数） |
| `SECRET_KEY` | 无 | 敏感字段加密密钥；未设置时明文存储（仅限开发） |

环境变量不会自动加载 `.env`，运行前请自行导出。

## 测试说明

- **后端**：`cargo test --all-targets`。125 项：`src/` 单元测试 110 项（config、crypto、db 迁移与 `ensure_columns`、app_settings、auth、cron（parser/scheduler/worker/log_capture/log_repository/seed）、logs_cleanup、routes 等）+ `tests/` 集成测试 15 项（auth 6、cron_jobs 8、notes 1）。
- 依赖全局 tracing subscriber 的测试（`cron::log_capture` 与 worker 日志链路）用 `SUBSCRIBER_LOCK` 串行执行；worker 日志测试须用 `current_thread` runtime（`set_default` 是线程局部的）。
- 环境变量隔离用 `temp-env`，临时目录用 `tempfile`；关停测试用 `tokio` 的 `test-util`（`start_paused` 虚拟时钟）。
- **前端**：`cd web && pnpm test run`。19 个文件 54 项，分布于 `src/__tests__/`（页面级）、`src/components/__tests__/`、`src/hooks/__tests__/`、`src/lib/__tests__/`（api）、`src/i18n/__tests__/`（中英键集合一致性 + 源码 `t()` 引用存在性）。
- `web/src/test/setup.ts` 为 Node 26 与 jsdom 的全局 `localStorage` 冲突做了内存 polyfill，并为 `ResizeObserver`/`scrollIntoView`/`matchMedia` 补了 jsdom 缺失的桩。
- 没有 E2E 测试。

## 代码风格与开发约定

### 提交前置动作（强制门禁）

**任何代码提交前，必须先完成以下校验且全部通过（全绿），否则不允许提交**：

```bash
cargo fmt                          # 后端全库格式化（非局部）
cargo clippy --all-targets --all-features -- -D warnings   # 修复到零警告
cargo test --all-targets           # 后端全量测试
cd web && pnpm lint                # 前端 biome check .
cd web && pnpm test run            # 前端全量测试
```

- 以上即 CI 门禁（`.github/workflows/ci.yml`：`cargo fmt --check` + `clippy -D warnings` + `cargo test --all-targets`；前端 biome + vitest + build）。
- 仓库提供版本控制的钩子脚本 `scripts/pre-commit.sh`（与上面同一套检查），安装一次即可自动执行：`ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit`。
- **既有代码引发的差异/警告/错误也必须一并修复**，不允许带警告提交，也不要回退 rustfmt/clippy 版本或跳过检查。
- **ADR/域文档与代码现状对齐**：提交前查看本次改动涉及的决策文档（`docs/adr/` 相关编号、`CONTEXT.md` 词条），若决策已实现、语义已演进或描述与代码现状不符，先更新文档再提交（文档可 `docs:` 单独提交）。

### Rust

- 使用 2024 edition。
- 统一 API 响应：`src/response.rs` 的 `Response<T>`，字段 `code`/`msg`/`data`，成功时 `code` 为字符串 `"0"`；错误码常量与 helper（`bad_request`/`not_found` 等）同在该文件，路由层统一使用。
- 错误处理：内部逻辑用 `anyhow::Result`，调度器另有 `SchedulerError`；HTTP 层一律返回统一 JSON 错误响应。
- 实体使用 SeaORM 派生宏；迁移写在 `src/db.rs::migrate()`，启动时自动建表并按 `schema_migrations` 版本号增量迁移。
- 数据库为 SQLite（WAL）。多语句事务里**把写放首位或把读移出事务**：DEFERRED 事务内先读后写会在并发提交时触发写升级竞态（`SQLITE_BUSY_SNAPSHOT`），`busy_timeout` 对此无效；相应回归测试必须用文件库（内存库没有 WAL 语义）。
- 日志：结构化 JSON，同时输出到 stdout 与 `logs/app.YYYY-MM-DD`。
- 定时任务 handler 通过 `scheduler.register_handler(name, ...)` 注册，与 `src/cron/seed.rs` 的种子行一一对应；未注册 handler 的任务在加载时被跳过。

### 前端

- 页面设计遵循 `web/DESIGN.md`（token 层、布局模式、i18n 规则、命名反模式），新增或修改页面前先读它。
- TypeScript 严格模式；路径别名 `@/` 指向 `web/src/`。
- UI 原语放 `web/src/components/ui/`（shadcn/ui 约定），类名用 `cn()` 合并；通用组件放 `web/src/components/`，页面放 `web/src/pages/`。
- 数据请求统一走 `web/src/lib/api.ts` 的 `api`（ky 实例）：`ApiResponse<T>` 对应后端信封，`code !== "0"` 抛 `ApiError`；错误文案统一经 `userErrorMessage()`，不要直接取 `error.message`。
- 状态管理以 TanStack Query 为主，Zustand 用于主题/语言等全局状态。
- i18n：文案一律走 `t()`，中英键集合由 `src/i18n/__tests__/locales-consistency.test.ts` 校验（缺键会直接渲染 key 原文）。
- 顶栏刷新按钮语义是**页面刷新**（`resetQueries` 清数据后立即重取，排除 `auth`/`health` 布局级键），不要在页面内再挂局部刷新按钮。

## 常见问题与注意事项

1. **前端构建产物必须存在**：发布构建时 `rust-embed` 会内嵌 `web/dist`。改完前端要先 `cd web && pnpm build` 再构建/重启后端，否则跑的还是旧界面。
2. **定时任务需要注册 Handler 才会执行**：库里存在但没有对应 handler 的任务，加载时被跳过（不出现在任务列表中，但可通过 API 删除）。
3. **环境变量不会自动加载 `.env`**：未集成 dotenv，运行前请确保已导出。
4. **日志清理**：启动后每天清理一次日志目录中超过 30 天的文件（按修改时间判断，不区分类型，不要往日志目录放其他文件）。
5. **Biome 配置**：`web/biome.json`（tab 缩进、双引号、100 列）。
6. **健康检查**：`/api/healthz` 只表示进程存活，不检查数据库等依赖。
7. **未知 API 路径返回 JSON 404**（`/api/` 前缀），其余路径回退 SPA 静态资源。

## Agent skills

### Issue tracker

Issues and specs are tracked in this repository's GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the five canonical triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

The repository uses a single-context domain-doc layout. See `docs/agents/domain.md`.

### Frontend design

Frontend pages follow the design rules in `web/DESIGN.md` (layout patterns, design tokens, i18n rules, named anti-patterns). Read it before adding or modifying pages.

### 代码质量审计

模块级审查与整改流程见 `docs/agents/audit.md`。

## CI tracking

- Hosting platform: GitHub Actions (`.github/workflows/ci.yml`).
- After every push, run `gh run watch` (or `gh run list --limit 5` / `gh run view <run-id>`) and report the outcome; do not assume success.
