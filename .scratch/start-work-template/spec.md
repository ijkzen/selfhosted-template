# Spec — selfhosted-template 模板化抽取（自 llm-gateway）

## Problem Statement

llm-gateway 是一个运行良好的 Rust + React 单体 LLM API 网关，但其全部业务逻辑（供应商、虚拟模型、API Key、协议转发、用量、数据面板等）与通用基础设施（服务器装配、登录会话、cron 引擎、设置、React 管理后台壳）耦合在一个代码库里。做新项目时无法复用这些久经验证的页面与接线，必须从零开始。用户需要一个通用单用户自托管后台模板：业务全部抽离，基础设施全部保留，新项目开箱即用、不用重写页面。

## Solution

把 llm-gateway 抽离为通用模板 selfhosted-template：后端保留 axum 装配骨架、SQLite 迁移框架、完整登录会话、设置域、cron 引擎、统一响应/加密/日志基础设施；前端保留登录页、布局/路由壳、shadcn/ui 组件库、API 客户端、双语 i18n 与主题；新增一个最小通用示例域 Notes 演示前后端 CRUD 骨架；命名/品牌统一为 selfhosted-template；`SECRET_KEY` 泛化；保留并适配 Docker/compose；质量工程（pre-commit、GitHub CI）覆盖 Rust 与 web 两个子项目。

## User Stories

1. 作为自托管项目开发者，我想要一个开箱即用的后台模板（登录、布局、导航、设置、示例 CRUD），以便我要做的新项目不用重写页面。
2. 作为开发者，我想要后端与前端骨架里没有任何 llm-gateway 领域残留（供应商/虚拟模型/API Key/转发/用量），以便模板语义干净、可以放心改名。
3. 作为开发者，我想要模板自带完整单用户认证（首次初始化建号、登录、cookie session、改密码、登出），以便任何自托管后台直接可用。
4. 作为开发者，我想要内置 cron 定时任务引擎（任务 CRUD、SSE 日志、时区语义），以便新项目接自动任务无需另起炉灶。
5. 作为开发者，我想要内置设置域（语言/时区键值设置）与双语 i18n 切换，以便多语言后台即刻可用。
6. 作为开发者，我想要一个通用示例域 Notes（列表 + 新建 + 编辑 + 删除 + 确认删除），以便照它的模式替换成自己的业务域。
7. 作为开发者，我想要 Dockerfile / compose.yaml 适配模板，以便新项目直接容器化部署。
8. 作为开发者，我想要质量工程覆盖前后端：pre-commit 跑 Rust（clippy、cargo test）与 web（Biome 检查、Vitest），GitHub CI 同样门禁，以便改动被自动化验收。
9. 作为开发者，我想要命名与品牌统一为 selfhosted-template（crate、web 包名、标题、主题/locale 存储 key），以便模板改名时无残留。
10. 作为开发者，我想要密钥环境变量泛化为 SECRET_KEY，以便任何需要加密存储的字段直接复用，而非网关专用语义。

## Implementation Decisions

- **范围**：从 llm-gateway 迁移"基础设施"，删除全部"业务逻辑"；**原样迁移，不做重构**（保持模块设计、命名惯例、测试模式）。
- **后端保留**：`main.rs`/`lib.rs` 装配骨架（tracing、graceful shutdown、DB 连接、AppState 仅含通用字段）、`config`（`BIND_ADDRESS`/`APP_ENV`/`DATABASE_URL`/`RUST_LOG`/cron 队列参数，+ 泛化 `SECRET_KEY`）、`db.rs` 迁移框架（建表清单裁剪为模板表）、`response` 统一信封、`i18n`、`middleware`、`static_assets`（rust-embed 内嵌 `web/dist`）、`logs_cleanup`、`crypto`（AES-256-GCM + 脱敏，密钥读 `SECRET_KEY`）、`app_settings`、`auth`（argon2/session/cookie/鉴权中间件，**去掉 API Key Bearer 分支**）、cron 框架（scheduler/worker/parser/repository/log_capture/SSE 全链路 + 三张 cron 表 + `routes/cron_jobs`，业务种子替换为通用示例 handler）。
- **后端保留实体**：user、session、setting、cron_job、cron_job_run、cron_job_log；**新增** note（示例域）。删除 provider/provider_template/provider_model/virtual_model/virtual_model_item/api_key/request/usage_cache 及全部业务模块与路由（providers/provider-models/virtual-models/api-keys/request-logs/stats/openai-compat/provider-templates/provider_repo/provider_model/view、proxy/、usage/、cron/seed 业务部分）。
- **后端耦合点处理**：`db.rs::migrate()` 建表清单与版本迁移重写为模板表；`lib.rs::init` 删除四处业务接线（backfill_api_key_hashes、upsert_templates、usage_refresh handler、ensure_usage_refresh_job），注册示例 cron handler；`state.rs` 删除 lb_state/usage_cache/upstream_pool 三个业务字段；`routes/mod.rs` 路由树裁剪为 auth/settings/cron_jobs/notes/healthz + fallback；`auth` 保留 `/api` cookie 分支。
- **REST 契约**：统一信封 `{code,msg,data}`；`/api/auth/*`（status/init/login/logout/me/change-password）、`/api/settings`、`/api/cron-jobs`（含 `logs/stream` SSE）、`/api/healthz`；新增 `/api/notes`（GET 列表 / POST 创建 / PUT 更新 / DELETE 删除，最小字段：id、title、content、created_at/updated_at 由后端维护）。
- **Cargo 依赖**：删除转发栈（hyper/http-body/http-body-util/tokio-rustls/webpki-roots）、reqwest、md-5/cbc/cipher/hmac/sha1/aes 等业务依赖；保留 axum、sea-orm/sqlx-sqlite、argon2、aes-gcm、tower-http、tracing 栈、serde、chrono/chrono-tz、uuid、rand、base64、thiserror、anyhow、tokio、tower、async-trait、sha2/hex（session 哈希）、croner、tokio-cron-scheduler、rust-embed、mime_guess。
- **前端保留**：`App.tsx` 路由分层（`/login` + `RequireAuth` + `AppLayout` + NotFound）、`login.tsx`、`not-found.tsx`、`layout.tsx`、`require-auth.tsx`、ky `lib/api.ts`、`lib/utils`、导航配置 `lib/pages.ts`（改造），20 个 shadcn/ui 组件、通用零件（data-table 列头/分页/列显隐、confirm-dialog、page-header、error/empty-state、search-input、status-badge、segmented-control、theme/locale toggle、skip-to-main、scroll-to-top 等）、hooks（use-auth/theme/locale/toast/mobile/in-view/init-settings）、i18n 双语基础设施命名空间、主题切换、vite/tsc/vitest/biome/tailwind 配置。
- **前端删除**：11 个业务页面（overview/provider-overview/virtual-model-overview/model-overview/providers/provider-models/virtual-models/api-keys/request-logs/cron-jobs/settings）、21 个业务 hook、业务组件目录与图表、业务 i18n 命名空间、业务测试。
- **前端改造点**：`App.tsx` 业务 lazy import 删除 + 新增 Notes 首页/路由；`lib/pages.ts` 导航替换（示例组 + 设置）；layout 品牌参数化；data-table/pagination、confirm-dialog、status-badge 中业务命名空间文案改为通用命名空间；存储 key（theme/locale）更名 `selfhosted-template-*`；标题、favicon 品牌。
- **命名**：crate `selfhosted-template`（lib `selfhosted_template`）、web 包名 `selfhosted-template-web`、品牌串/标题/主题 key 统一。
- **Docker/Compose（用户要求保留）**：根 `Dockerfile` 多阶段（node → `web/dist`，rust → 静态二进制）；根 `compose.yaml`、`.dockerignore`、`.env.example`（`SECRET_KEY`）适配模板。**不迁移** nightly/release/gitea/.deploy/deploy/scripts。
- **质量工程（用户要求适配 web）**：pre-commit hook 依次跑 clippy、cargo test、web Biome check、web Vitest（rust-embed 依赖 `web/dist`，CI 与构建流程中先构建 web）；`.github/workflows/ci.yml` 增加 web job（pnpm build → biome check → vitest run）并保证 rust job 前 `web/dist` 已产出。
- **测试 seam**：后端 HTTP 边界黑盒集成测试（沿用 llm-gateway `tests/common` 模式：临时 SQLite + 随机端口 + 完整路由树）；既有基础设施模块单测（config/crypto/db/auth/cron/logs_cleanup）保留；前端沿用组件级 `__tests__`（vitest + testing-library）。新增 notes 集成测试与 Notes 页面/表单测试。

## Testing Decisions

- 好测试的标准：只测外部行为，不测实现细节——后端走 HTTP 断言信封与状态码，前端走渲染与交互断言。
- 后端集成测试：沿用既有 `tests/common` 基建（真实 app 启动、临时 DB、mock 无关外部依赖），先测基础设施回归（登录/设置/cron CRUD/健康检查），再测 notes CRUD 全生命周期。
- 前端测试：为保留的通用组件（已有 `__tests__` 存在者原样保留）与新增 Notes 页面（列表渲染、新建/编辑表单提交、删除确认）补测。
- 验证链路：`cargo test --all-targets`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo fmt --check`、`cd web && pnpm test`、`pnpm build`（tsc 类型检查）。

## Out of Scope

- llm-gateway 全部业务域（供应商、虚拟模型、API Key、协议转发、用量/额度、数据面板/请求日志）——不迁移、不保留示例数据。
- 多用户/RBAC、OAuth 等认证扩展；仅保留单用户登录会话。
- 示例域之外的新页面/新组件；不引入泛型 DataTable 抽象、不做服务端分页。
- 发布/部署专用 CI（nightly/release/gitea）、.deploy/deploy/scripts、社区文档模板。
- 前端 websocket/SSE 之外的实时能力无需新增。
- 不做架构重构（数据库、消息中间件、部署目标均沿用 llm-gateway 的现状）。

## Further Notes

- 迁移以 llm-gateway 当前工作区为唯一事实来源；删除的业务代码不再保留副本（git 历史可回溯）。
- rust-embed 在编译期读取 `web/dist`，开发/CI/容器构建的**顺序约束**：先 `cd web && pnpm build` 再 cargo 构建。
- CONTEXT.md / docs/adr/0001 已记录模板领域词汇与抽取决策。