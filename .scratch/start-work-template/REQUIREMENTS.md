# Requirements — selfhosted-template 模板化抽取

请求原文（用户）：llm-gateway 是一个运行良好的网关项目，抽离它所有的业务逻辑，保留它的后端基础设施和前端基础设施，放到本项目中，让本项目成为一个模板项目，以后做新项目就不用重新再写页面了。

## 范围

把 `/Users/ijkzen/Projects/RUST-Project/llm-gateway` 的**全部业务逻辑**抽离，保留**后端基础设施**与**前端基础设施**，迁移进 `/Users/ijkzen/Projects/RUST-Project/selfhosted-template`，做成通用**单用户自托管后台模板**。

### 保留的后端基础设施

- 服务器装配骨架：`main.rs`/`lib.rs`（tracing 双通道日志、graceful shutdown、DB 连接）、`config`、`db.rs` 迁移框架、`response`（统一信封）、`i18n`、`middleware`（CORS/trace/catch-panic）、`static_assets`（rust-embed 内嵌前端）、`logs_cleanup`
- 完整单用户认证：`auth`（argon2 + cookie session + 鉴权中间件，去掉 API Key Bearer 分支）、`entity/user`、`entity/session`、`routes/auth`、初始化建号流程
- 设置域：`entity/setting`、`app_settings`、`routes/settings`（去掉时区/语言更新里的 cron 业务钩子之外的部分——钩子因保留 cron 而保留）
- Cron 框架：`cron/`（scheduler/worker/parser/repository/log_capture/SSE）+ 三张表（cron_job、cron_job_run、cron_job_log）+ `routes/cron_jobs`；种子/示例 handler 替换为通用示例
- `crypto`（AES-256-GCM + 脱敏，密钥环境变量泛化为 `SECRET_KEY`）

### 保留的前端基础设施（web/）

- 路由分层壳：`App.tsx`（`/login` + `RequireAuth` + `AppLayout` + NotFound）、`login.tsx`、`not-found.tsx`、`layout.tsx`、`require-auth.tsx`
- ky API 客户端 `lib/api.ts`、`lib/utils`、`lib/pages`（导航配置改造）
- 20 个 shadcn/ui 组件、通用零件（data-table 列头/分页/列显隐、confirm-dialog、page-header、error/empty-state、search-input、status-badge、segmented-control、theme/locale toggle、skip-to-main、scroll-to-top 等）
- hooks：`use-auth`、`use-theme`、`use-locale`、`use-toast`、`use-mobile`、`use-in-view`、`use-init-settings`
- i18n 双语骨架（zh-CN/en 基础设施命名空间）+ 语言切换；主题（亮/暗）切换
- 全量构建/测试基建（vite/tsc/vitest/biome/tailwind/shadcn 配置）

### 新增：示例域（Notes）

最小通用 CRUD 示例，前后端完整：
- 后端：`entity/note` + `routes/notes`（列表/创建/更新/删除），接入 db 迁移与路由树
- 前端：Notes 页面演示列表 + 新建/编辑弹窗 + 删除确认（复用现有表格零件），挂到导航
- 测试：笔记 CRUD 后端集成测试 + 前端组件/页面测试

### 保留并适配：Docker / Docker Compose（用户补充要求）

- 根 `Dockerfile` 多阶段构建：**先 `cd web && pnpm build` 产出 `web/dist`（rust-embed 编译期硬依赖），再 cargo build**；运行镜像含二进制即可
- 根 `compose.yaml` 与 `.dockerignore` 适配模板（服务名、卷、`SECRET_KEY` 环境变量、`APP_ENV=prod` 路径约定）
- `.env.example` 按模板重写（`SECRET_KEY` 等）

### 质量工程：web 子项目适配（用户补充要求）

- **pre-commit hook**：在现有 clippy + cargo test 基础上增加 web 质量门（`pnpm -C web` 的 lint/format 与 `vitest run`）；注意 rust-embed 要求 `web/dist` 存在，hook 与 CI 中先把 web 构建/检查纳入
- **GitHub CI（模板 `ci.yml` 适配）**：增加 web job（安装 pnpm → `pnpm build`（供 rust-embed）→ biome lint/format check → `vitest run`），并在 rust job 前确保 `web/dist` 已构建（或同 job 内先 build web 再 cargo clippy/test）
- AGENTS.md 的 CI tracking 段同步（可在实现时微调命令）

## 已确认决策

1. Cron 框架：**保留**（含依赖、表、SSE 全链路）
2. 示例域：**通用示例 CRUD（Notes）**
3. 命名/品牌：**统一 selfhosted-template**（crate、web 包名、标题、主题/locale 存储 key）
4. 认证：**保留完整登录**（init + login + session + 改密码）
5. i18n：**保留双语**
6. 改造幅度：**原样删业务，不重构**
7. 密钥：`API_KEY_ENCRYPTION_KEY` → **`SECRET_KEY`** 泛化
8. 测试：保留基础设施单测（config/crypto/db/auth/cron/logs_cleanup）+ **补示例域测试**

## 非目标（ponytail 裁剪）

- ❌ 不迁移 llm-gateway 的发布/部署专用残留：`nightly.yml`/`release.yml`/`.gitea`/`.deploy`/`deploy`/`scripts`/`.github` 社区模板文件（Dockerfile/compose/.dockerignore/.env.example 因用户要求**保留并适配**）
- ❌ 示例域不过度设计：不用泛型 `DataTable<T>`、不做服务端分页，Notes 列表一次拉全量，直接复用现有表格零件
- ❌ 骨架不加新抽象、不改模块设计；仅做"删业务 + 改耦合点"
- ❌ 不新建 CI（模板 `ci.yml` 保留并适配新骨架 + web 子项目质量门）
- ❌ 业务专用 Cargo 依赖随业务删除（hyper 转发栈、reqwest、md-5/cbc/cipher/hmac/sha1/aes 等）

## 待答卷（to-spec/to-tickets 阶段）

- 迁移执行顺序与文件级清单（实现时按现有盘点推进）
- Notes 实体字段与 REST 形状（最小可用即可）
- 集成测试源用 llm-gateway `tests/common` 基建的模式改造（已确认"保留+补测试"，具体列表实现时定）