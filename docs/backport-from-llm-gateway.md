# llm-gateway 基础设施沉淀计划

## 背景

模板于 2026-09-02 从 llm-gateway 抽离（ADR-0001）。此后 llm-gateway 继续演进，其中 2026-09-08 ~ 09-12 的一轮「代码质量整改」（wayfinder 模块审查 21 票 + 四维审计 FINDINGS 25 项）里有相当一批是**基础设施层修复**，与 LLM 网关业务无关，模板停留在抽离时的代码，等于继承了同一批缺陷。

本文档记录经逐条比对后判定为**业务无关、可沉淀**的项，作为迁移计划与进度台账。业务专属项（proxy 协议转换、usage 抓取、stats 快照、request_logs、provider/虚拟模型/API Key、chat、backup、availability、failure_recovery）不在范围内。

## 迁移原则

1. **不照抄迁移编号**：两边迁移体系不同——模板 migration 5 ≡ gateway migration 4，gateway 的 25 号迁移在本模板应落为 6 号。
2. **保留模板的泛化命名**：如 `SECRET_KEY`（gateway 为 `API_KEY_ENCRYPTION_KEY`）、`session_cookie()` helper（gateway 已内联删除）。
3. **只搬业务无关部分**：gateway 提交里混着业务改动时，只取基础设施那部分。
4. **每批次收尾必须跑验证**：后端 `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test --all-targets`；前端 `tsc -b` + `biome check` + `vitest run` + `vite build`。
5. **模板已有而 gateway 没有的不回退**：见文末「反向差异」。

## 状态总表

| ID | 项 | 批次 | 状态 |
|----|----|------|------|
| B1 | 优雅关停收尾超时只在信号到达后计时 | B | 已完成 |
| B2 | 首启初始化改原子写入（消除并发双初始化） | B | 已完成 |
| B3 | crypto：`mask` 按字符计数 | B | 已完成 |
| B4 | crypto：`decrypt_or_passthrough` 解密失败按不可用处理 | B | 不适用（改判） |
| B5 | auth：路径尾斜杠归一 | B | 已完成 |
| B6 | auth：logout 幂等且免登录 | B | 已完成 |
| B7 | auth：Cookie `max_age` 用 `SESSION_TTL_SECS` 常量 | B | 已完成 |
| F1 | 前端：`beforeError` 保留错误身份（修 ky 重试白名单失效） | F | 已完成 |
| F2 | 前端：`userErrorMessage` 统一错误文案（超时/网络/取消） | F | 已完成 |
| F3 | 前端：请求超时 10s → 30s | F | 已完成 |
| C1 | cron：日志事件 `Arc` 化 + 去掉 std mpsc 桥接 | C | 已完成 |
| C2 | cron：捕获侧 per-span 单调 `seq` | C | 已完成 |
| C3 | cron：日志攒批落库 + 失败重试 + 缓冲上限 | C | 已完成 |
| C4 | cron：run 孤儿与永久 running 双修 | C | 已完成 |
| C5 | cron：SSE 先订阅后读快照 | C | 已完成 |
| C6 | cron：prune 阈值直删 + `(run_id, seq)` 覆盖索引 | C | 已完成 |
| C7 | cron：列表字母序 / 未加载任务可软删 / next_run 单一写者 | C | 已完成 |
| C8 | cron：测试等待改事件同步，消除负载敏感抖动 | C | 已完成 |
| D1 | app_settings：种子行先于解析、`reset_key`、`TIMEZONE_SYNC` | D | 部分完成（后两项改判不适用） |
| D2 | db：`ensure_columns` helper、`is_unique_violation` 上移、迁移测试拆分 | D | 部分完成（拆分改判不适用） |
| D3 | routes：未知 `/api/` 路径返回 JSON 404 | D | 已完成 |
| FE1 | 前端：cron SSE 退避重连 + `reconnectExhausted` | FE | 已完成 |
| FE2 | 前端：i18n 首帧 lang / 入口初始化 / 语言热切换 / 键集合测试 | FE | 已完成 |
| FE3 | 前端：顶栏刷新改 `resetQueries` 语义 | FE | 已完成 |
| FE4 | 前端：登录回原页、`/login` 全局 Suspense、vite 分包、码点守卫、matchMedia 桩 | FE | 已完成 |
| E1 | CI：`cargo fmt --check`、`--all-targets`、rust-cache | E | 已完成 |
| E2 | AGENTS.md 补工程约定（门禁/风格/结构树/1000 行） | E | 已完成 |
| E3 | 补 README 与 `docs/user-guide.md` | E | 已完成 |
| E4 | 沉淀审计方法论 `docs/agents/audit.md` | E | 已完成 |
| E5 | `.gitignore` 补 ZCode 工作目录；pre-commit 纳入版本控制 | E | 已完成 |

## 分项明细

### 批次 B：后端 P0（安全与正确性）

**B1 优雅关停收尾超时**（gateway `bb319b0`）
模板 `src/lib.rs` 直接 `with_graceful_shutdown(shutdown_signal())`，收尾没有上限，长连接（SSE）可将进程钉死，`scheduler.stop()` 与 worker 收尾永不执行。gateway 改为 `serve_until_shutdown(serve, shutdown, drain_timeout)`，`HTTP_DRAIN_TIMEOUT_SECS = 8`，且**超时只在信号到达后计时**（早期版本把 timeout 套在整个 serve 上，导致无信号也满 8 秒自退、容器反复重启）。
验证：`tokio` dev-dep 加 `test-util`，用 `start_paused` 虚拟时钟写「信号到达前不得退出」回归测试。

**B2 首启初始化原子化**（gateway `12` 票 review）
模板 `src/routes/auth.rs` 先 `count` 再插入（check-then-act），并发首启可建出两个用户。gateway 改为 `INSERT ... SELECT ... WHERE NOT EXISTS (SELECT 1 FROM user)` 单语句。模板是单用户假设，直接命中。

**B3/B4 crypto**
`mask` 按字节判长却按字符切片，4 个汉字（12 字节）会通过长度检查被完整回显 → 改按 `chars().count()`。已实施，并补 `mask_counts_characters_not_bytes` 回归测试。
**B4 改判为不适用**：模板的 crypto 模块在自身之外没有任何调用方，也没有 `decrypt_or_passthrough` 这个函数（它是 gateway 为「调用方要 String 而非 Result」场景加的包装）。模板的 `decrypt` 对「带前缀但解不开」本来就返回 `Err`，语义已是 fail-closed，无需再引入一个无消费者的包装函数。

**B5/B6/B7 auth 中间件**
路径尾斜杠归一（`path.trim_end_matches('/')`），否则 `/api/auth/status/` 落进「需登录」分支；logout 提为公开且幂等，会话已过期时也能清 Cookie；Cookie `max_age` 改用 `SESSION_TTL_SECS` 常量。

**批次 B 验证**：`cargo clippy --all-targets --all-features -- -D warnings` 无告警；`cargo test --all-targets` 112 项全绿（97 单元 + 15 集成，含 B1 新增 4 项、B3 新增 1 项）。

### 批次 F：前端 P0（错误链路）

**F1/F2/F3**（gateway `19-02/03/04`，提交 `7674205`）
模板 `web/src/lib/api.ts` 的 `beforeError` 用 `new ApiError` 替换错误对象身份，导致 ky 的 `isHTTPError` 判定失效——重试白名单与 `Retry-After` 尊重被绕过；网络/超时错误不经过 `beforeError`，toast 显示英文原文；`timeout: 10000` 偏短。
已改为：保留错误身份并挂 `apiCode`；新增 `apiCodeOfError` / `userErrorMessage`（TimeoutError 带 method/path、TypeError → 网络文案、AbortError → 已取消）；超时 30s；ky `retry` 归零交由 react-query 的 `retry: 1` 承担（避免两层叠加打满 4 次）。`use-toast.ts` 的描述改用 `userErrorMessage`，新增 i18n `error.networkError` / `error.timeout` / `error.aborted`。
顺带落地 FE4 的 api.ts 半边：`AUTH_REDIRECT_FROM_KEY` + `readStoredRedirectFrom()`（读取即消费），登录页接线仍在批次 FE。

**批次 F 验证**：`tsc -b` 与 `biome check` 无输出（干净）；`vitest run` 18 文件 51 项全通过（新增 `src/lib/__tests__/api.test.ts` 10 项）。

### 批次 C：cron 引擎（后端最重）

**实施方式**：`cron/log_capture.rs`、`log_repository.rs`、`worker.rs`、`scheduler.rs` 四个文件整体对齐 gateway 版本（模块内除 seed 外全为基础设施），再补回模板自身已有的便捷构造 `JobWorker::new` / `SchedulerRuntime::new`（模板测试大量使用，gateway 已删除，属「模板更新」不回退）。模板自有的 `parser.rs` 便捷包装同样保留。

**C1 `Arc` 化 + 去桥接**（gateway `edd467c`、`808c53f`）
`src/lib.rs` 的 `spawn_blocking` + std mpsc 桥接整体删除，`JobLogLayer` 直连 `broadcast::Sender<Arc<JobLogEvent>>`；`state.rs`、`routes/cron_jobs.rs`、`tests/common/mod.rs` 同步类型。

**C2 per-span 单调 seq**（gateway `92cd523`）
`log_capture.rs` 的 span 表改存 `(job_name, run_id, next_seq)`，在同一把锁内自增分配，与 worker 落库序同源（广播 FIFO 保序）。

**C3 日志攒批落库**（gateway `3b4111c`）
trait 删 `insert_log` 换 `LogRow` + `insert_logs`；worker 侧 `RunLogSink` 攒 50 条或 100ms 空闲 flush，flush 失败保留缓冲重试（成功才推进 seq/计数），4000 条上限丢最旧并置 truncated，`Lagged` 转截断提示。

**C4 run 孤儿与永久 running**（gateway `9747750`）
`run_persisted` 门控 sink、新增 `delete_orphan_logs`（`lib.rs::init` 启动时调用）、`finish_run` 仅对 `running` 生效、`prune_old_runs` 顺带回收超 6h 的 running。

**C5 SSE 先订阅后读快照**（gateway `da4811d`）
`routes/cron_jobs.rs` 把 `subscribe()` 提到 `list_runs` 之前。

**C6 prune 直删 + 覆盖索引**（gateway `4e32af9`、`7daf6d7`）
`prune_old_runs` 改 `limit(keep + 1)` 取阈值、事务外先取待删 run_id。新增**迁移 6**：建 `idx_cron_job_logs_run_seq(run_id, seq)` 并 drop 单列 `idx_cron_job_logs_run_id`（gateway 对应迁移 25，编号已按模板体系重排）。

**C7 三个小修**
`list_jobs` 按 group + name 排序；`soft_delete_job` 对未加载任务退化为纯 DB 软删；`update_job` 仅表达式变更才重算 next_run_at，并在内存更新失败时回写旧定义；计划推进收敛到 `scheduler::on_run_finished` 单一实现（worker 只报告事实）。

**C8 测试抖动根治**
worker 测试的固定预算 `sleep(300ms)` 全部换成 `wait_for_run` 轮询 helper + `set_default` subscriber guard。模板已知的「并行 `cargo test --all-targets` 偶发失败」由此消除。

**批次 C 验证**：`cargo clippy --all-targets --all-features -- -D warnings` 无告警；`cargo test --all-targets` 连续三轮全绿，每轮 124 项（109 单元 + 15 集成）；确认四个文件**无测试函数被删除**、新增 12 项。

### 批次 D：后端其他

**D1 app_settings**
`load_from_db` 改为先调 `seed_rows(db)` 再解析：否则首启时进程内 timezone 解析到 `None`，cron 链路回退服务器本地时区，而库里已写入默认时区，两口径分叉一个启动周期。
**改判不适用两项**：① `reset_key` —— 模板缓存键只有 language / timezone，而 `DELETE /api/settings/{key}` 本就拒绝删除这两个受保护键，没有任何键需要「删后恢复默认值」，加进去就是无消费者的 API；② `TIMEZONE_SYNC` —— gateway 的 `timezone_sync()` 消费者全在业务模块（proxy 会话派生、usage fetcher、stats 快照），模板只有 `lang_sync()` 一个同步上下文消费者（panic 中间件），时区不需要同步副本。

**D2 db 层**
新增 `ensure_columns(db, version, columns)` 收敛「缺列补 ALTER」样板，migration 2 改用它；配 `ensure_columns_adds_all_missing_columns_in_one_version` 回归测试（同一版本内两列都缺必须一次补齐——分两次调用同版本会被版本守卫吞掉第二列，这正是 gateway 踩过的坑）。
迁移守卫注释按 gateway 修正：in-transaction guard 的作用是让并发第二个迁移者 **fail-fast**（DDL 随事务回滚，库不会半迁移），**不是**串行化并发首启迁移。
`db::connect` 补 WAL 写升级竞态说明（DEFERRED 事务首语句必须是写，`busy_timeout` 无效，回归测试须用文件库），并把 `cache_size` 注释的数值错误修正为 62.5 MiB/连接。
**改判不适用一项**：迁移测试拆分到 `src/db/tests.rs` 的理由是对齐「单文件 ≤1000 行」，模板 `db.rs` 现为 380 行，拆分没有收益。

**D3 未知 API 路径 JSON 404**
`routes/mod.rs` 的 fallback 从裸 `static_assets::serve_asset` 换成 `api_aware_fallback`：`/api/` 前缀的不存在路径返回 JSON 404（按当前语言本地化），其余仍走 SPA 静态资源。拼错 API 路径不再拿到 `index.html` 的 200。

**批次 D 验证**：`cargo clippy --all-targets --all-features -- -D warnings` 无告警；`cargo test --all-targets` 125 项全绿（110 单元 + 15 集成）。

### 批次 FE：前端 P2

**FE1 cron SSE 退避重连**（gateway `18-11`、`18-05`、`18-02`）
`use-cron-job-logs.ts` 整体对齐 gateway：`onerror` 自带 2^n 退避（上限 30s）+ 5 次上限，超限置 `reconnectExhausted` 并停连；`snapshot` / `idle` 时失效历史列表（补断线期间错过的 `run_ended`）；`reset` 时用 `staleTime: 0` 拉全量并按 seq 合并（避免把拉取期间新到的增量整体替换掉）。`CronJobLogsDialog` 在「重连中」旁增加中断提示 + 刷新按钮，新增 i18n `cronJobs.reconnectFailed`。

**FE2 i18n**
`languageChanged` 处理器注册提到 `init` 之前，并在 init 后直接按初始语言落 `<html lang>`（init 内的 languageChanged 在处理器注册前同步触发，事件会漏掉首帧）；`main.tsx` 显式 `import "@/i18n"`；`use-settings.ts` 的 `useUpdateSetting.onSuccess` 在直改 `language` 时同步 `setLocale` + `i18n.changeLanguage`（否则界面语言与设置表长期分叉）。
新增 `src/i18n/__tests__/locales-consistency.test.ts`（中英键集合、占位符集合、源码 `t()` 引用存在性），配套补 `src/vite-env.d.ts`（`import.meta.glob` 的类型来源，模板此前缺失）。
**该测试当场抓到一个真实缺键**：`components/notes/note-form.tsx` 引用 `common.create` 但 locales 无此键（按钮渲染出 key 原文），已补中英文案。

**FE3 顶栏刷新语义**（gateway `049a7d6`）
`layout.tsx` 的刷新按钮从 `invalidateQueries()`（只标脏不删数据）改为 `resetQueries({ predicate })`（排除 `auth` / `health` 布局级键），并加 `useIsFetching` 忙碌态禁用 + 图标旋转。沿用模板内联写法，未引入 `PageRefreshButton` 组件（该抽取仍属既定不搬项）。

**FE4 零散小修**
登录页改用 `state.from ?? readStoredRedirectFrom() ?? "/"`（会话过期由 HTTP 层整页跳转过来时没有 router state）；`App.tsx` 增加包住整棵路由树的全局 `Suspense`（`/login` 在 layout 之外，硬刷新会白屏）；`vite.config.ts` 的 `manualChunks` 改匹配 `node_modules/react-router`（原来只匹配 `react-router-dom`，核心包落默认 chunk）；`middleEllipsis` 守卫改按码点计数；`test/setup.ts` 补 `matchMedia` 桩。

**批次 FE 验证**：`tsc -b` 与 `biome check` 无输出；`vitest run` 19 文件 54 项全通过；`vite build` 成功且无告警（`router` chunk 由空壳变为 37.19 kB，分包修复生效）。

### 批次 E：工程规范

**E1 CI**：rust job 加 `cargo fmt --check`（toolchain 补 `rustfmt` 组件）、加 `Swatinem/rust-cache@v2`、`cargo test` 改 `--all-targets`，job 名同步为 `rust (fmt + clippy + test)`。落地前先跑 `cargo fmt` 全库对齐。

**E2 AGENTS.md**：从 22 行扩为完整工程约定——项目概述、技术栈、结构树（含「单文件 ≤1000 行，超限按域拆子模块」）、构建与运行、环境变量表、测试说明（含真实计数与串行/`current_thread` 等注意事项）、提交前置门禁五连 + `scripts/pre-commit.sh` 安装方式、Rust / 前端代码风格、常见问题。业务章节（供应商、协议、调度器业务语义、CORS 口径）不搬。

**E3 文档缺口**：新增 `README.md`（功能特性、架构、快速开始、Docker、环境变量表、测试、文档索引、用模板开新项目的三步）与 `docs/user-guide.md`（界面逐项说明：初始化、侧栏、刷新语义、笔记示例域、定时任务含日志弹窗、设置与账号、自动化行为、术语速查）。内容按模板实际实现重写，不照搬 gateway 业务章节。

**E4 审计方法论**：新增 `docs/agents/audit.md`——核心纪律（审改分离、行号逐条复核、每条带证据）、P1-P3 严重度定义、`.scratch/<effort>-<date>/{map.md,MODULES.md,issues/,findings/}` 目录约定、七步流程（模块边界盘点 → 分票 → 四轴 + 性能内存轮 → FINDINGS → 拍板 → 归位 → 分批实施）、FINDINGS 条目模板、与 wayfinder / grilling / code-review / diagnosing-bugs 的分工。`AGENTS.md` 的「Agent skills」节加了指向。

**E5 工作目录与钩子**：`.gitignore` 补 `/.worktrees`、`/.zcode/plans`、`/.zcode/tmp`；pre-commit 落为版本控制的 `scripts/pre-commit.sh`（检查项与 CI 对齐，含 `cargo fmt --check` 与 `cargo test --all-targets`），并把 `.git/hooks/pre-commit` 换成指向它的软链，本地与 CI 从此同源。

## 整体验证

全部批次完成后按仓库门禁跑了一遍，并补做真实运行冒烟（内存库测试覆盖不到迁移与关停）：

| 检查 | 结果 |
|------|------|
| `cargo fmt --check` | 无输出（干净） |
| `cargo clippy --all-targets --all-features -- -D warnings` | 无告警 |
| `cargo test --all-targets` | 125 项全绿（110 单元 + 15 集成），连续三轮无抖动 |
| `cd web && npx tsc -b` | 无输出（干净） |
| `cd web && npx biome check .` | 110 文件，无问题 |
| `cd web && npx vitest run` | 19 文件 54 项全通过 |
| `cd web && npx vite build` | 成功，无告警 |
| 真实运行冒烟 | 迁移 1-6 全部应用、`idx_cron_job_logs_run_seq` 就位且旧单列索引已删；`init` 建号成功；带会话访问未知 `/api/*` 返回 JSON 404、SPA 路径仍 200 html；设置与定时任务列表可读（种子任务 `next_run_at` 落在未来时刻）；`SIGTERM` 后 0.23 秒干净退出 |

## 反向差异（模板比 gateway 新，不得回退）

- 模板 CI 有独立 web job 跑 `biome` + `vitest`；gateway CI 只 build 前端，不跑这两项。
- 模板有 pre-commit 钩子（web build → clippy → test → biome → vitest）；gateway 无。
- 模板有 `web/DESIGN.md` 设计规范；gateway 无。
- `SECRET_KEY` 命名比 gateway 的 `API_KEY_ENCRYPTION_KEY` 更通用。
- `cron/parser.rs` 保留 `compute_next_run` 等便捷包装（gateway 当死代码删除）。
- `auth/mod.rs` 保留 `session_cookie()` helper（gateway 已内联删除）。
- 迁移编号体系不同（模板 1 是 session 过期索引，gateway 1 是 cron 列）。

## 不在范围内（业务专属）

proxy 协议转换与流式转运、usage 抓取与额度门控、stats / stats_snapshot、request_logs、provider / virtual model / API Key、chat、backup、availability、failure_recovery；`.gitea/`、`.deploy/`、私有 registry、FRP 部署脚本。
