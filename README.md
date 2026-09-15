# selfhosted-template

[![CI](https://github.com/ijkzen/selfhosted-template/actions/workflows/ci.yml/badge.svg)](https://github.com/ijkzen/selfhosted-template/actions/workflows/ci.yml)

通用单用户自托管后台模板：Rust（axum + SeaORM + SQLite）单体 + React 管理后台，前端产物由 `rust-embed` 内嵌进单个二进制。

由 [llm-gateway](https://github.com/ijkzen/llm-gateway) 抽离全部业务逻辑后形成（见 `docs/adr/0001-template-from-llm-gateway.md`）：保留久经验证的前后端基础设施，去掉供应商/模型/转发/用量等业务域。新项目以它为起点，把示例域 Notes 换成自己的业务即可。

## 功能特性

- **登录与初始化**：argon2id 密码哈希、服务端 Cookie 会话（令牌哈希入库、可踢旧会话）、首启引导建号（原子写入，并发安全）、改密后吊销其他会话。
- **定时任务引擎**：标准 Cron 与 `@every` 间隔（croner），分组与启停管理、执行历史与日志持久化、日志实时 SSE 推送（per-run 单调 seq 去重 + 断线退避重连）、超期执行自动回收。
- **设置**：键值设置表（类型化校验），内置语言与时区两项；语言切换前后端同步，时区变更重建全部定时任务。
- **双语 i18n**：后端消息与前端界面同源双语（zh-CN / en），中英键集合一致性由测试锁定。
- **统一响应与错误**：`{code, msg, data}` 信封，前端错误文案统一映射（超时/网络/取消），401 自动跳登录并在登录后回跳原页面。
- **加密与掩码**：`SECRET_KEY` 驱动的 AES-256-GCM 加解密与安全掩码工具，供新业务存储敏感字段。
- **工程化**：SQLite WAL + 版本化迁移、结构化 JSON 日志与定期清理、优雅关停（收尾超时）、CI 门禁（fmt + clippy + test + biome + vitest + build）、版本化 pre-commit 钩子。

## 架构

```
浏览器 ──/api/*──> axum 路由 ──> SeaORM ──> SQLite（WAL）
   │                  │
   │                  └── 会话中间件（Cookie）+ 统一响应信封
   └──/（SPA）──> rust-embed 内嵌的 React 构建产物
```

前端为单页应用，`/api/*` 之外的所有路径回退到内嵌静态资源；`/api/` 下不存在的路径返回 JSON 404。

## 界面预览

> 首次运行后截图补充：登录页、首页、定时任务、设置。

## 快速开始

### 前置

- Rust（stable，2024 edition）
- Node.js 22+ 与 pnpm
- 无 Docker 也可本地运行（Dockerfile 与 compose.yaml 已提供）

### 本地运行

```bash
# 1. 构建前端（rust-embed 在编译期嵌入 web/dist，改动前端后必须重新构建）
cd web && pnpm install && pnpm build && cd ..

# 2. 启动服务
export BIND_ADDRESS=127.0.0.1:4007 \
       APP_ENV=dev \
       DATABASE_URL='sqlite://db/app.db?mode=rwc'
cargo run
```

首次访问会进入初始化页，创建首个用户（单用户模型）。初始化完成后即可登录后台。

### Docker

```bash
docker build -t selfhosted-template .
docker run -d --name selfhosted-template \
  -p 4007:4007 \
  -v "$PWD/config/db:/config/db" \
  -v "$PWD/config/logs:/config/logs" \
  -e SECRET_KEY='<随机串>' \
  selfhosted-template
```

`SECRET_KEY` 用于敏感字段加密：未设置时明文存储（仅限开发），设置后新写入的敏感值自动加密。**密钥轮换会导致旧密文无法解开**，请与数据库一同备份。

### Docker Compose

```bash
docker compose up -d --build
```

## 配置

| 变量 | 默认 | 说明 |
|------|------|------|
| `BIND_ADDRESS` | `0.0.0.0:4007` | 监听地址 |
| `APP_ENV` | `dev` | `dev` / `prod`；非法值静默回退 `dev` |
| `DATABASE_URL` | dev: `sqlite://db/app.db?mode=rwc` | SQLite 连接串，父目录自动创建 |
| `RUST_LOG` | `info,sqlx::query=warn` | 日志过滤 |
| `CRON_JOB_QUEUE_SIZE` | `1000` | 任务队列容量（正整数） |
| `CRON_JOB_MAX_CONCURRENT` | `10` | 并发执行上限（正整数） |
| `SECRET_KEY` | 无 | 敏感字段加密密钥（未设置则明文存储） |

环境变量**不会**自动加载 `.env`，请自行导出或通过容器注入。

## 测试

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets          # 后端 125 项

cd web && pnpm lint && pnpm test run   # 前端 54 项
```

提交前请跑完整门禁，或安装版本化钩子自动执行：

```bash
ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
```

## 文档

| 文档 | 内容 |
|------|------|
| `AGENTS.md` | 工程约定：构建、测试、代码风格、提交门禁、常见问题 |
| `docs/user-guide.md` | 使用者手册（界面与功能逐项说明） |
| `CONTEXT.md` | 域词汇表 |
| `docs/adr/` | 架构决策记录 |
| `docs/agents/` | 给 agent 的规范：issue tracker、triage 标签、域文档布局、代码质量审计流程 |
| `docs/backport-from-llm-gateway.md` | 从 llm-gateway 沉淀基础设施改动的计划与进度台账 |
| `web/DESIGN.md` | 前端设计规范（token、布局、i18n、反模式） |

## 用这个模板开新项目

1. 用 `rename` skill 改项目名（crate / 包名 / 镜像名 / 品牌文案 / 存储键，含完整清单）。
2. 读 `web/DESIGN.md` 与 `CONTEXT.md`，把示例域 Notes 换成自己的业务域（entity / route / page / hook / i18n / tests 六件套可直接照抄）。
3. 在 `src/lib.rs::init` 注册自己的定时任务 handler，并在 `src/cron/seed.rs` 加对应种子行。
