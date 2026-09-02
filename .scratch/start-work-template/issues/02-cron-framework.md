# 02: cron 框架迁移

**What to build:** 把 llm-gateway 的 cron 定时任务引擎全链路迁移为模板通用能力：调度器（cron 表达式 + `@every` + 时区语义）、worker（信号量并发）、parser、repository/log_repository/log_capture（SSE 日志流）、三张表（cron_job/cron_job_run/cron_job_log）、routes/cron_jobs（CRUD + run + 日志查询 + `logs/stream`）。删除网关业务种子（usage_refresh），替换为一个通用示例 handler 注册；settings 的时区/语言更新钩子保留。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] cron 模块与三张表迁移完成并接入 db 迁移
- [ ] 路由树含 /api/cron-jobs（CRUD + run + SSE）
- [ ] 示例 cron handler 注册，业务种子删除
- [ ] cron 相关单测（parser/scheduler/worker/repository/log_capture）迁移并全绿
- [ ] `cargo test --all-targets` / clippy / fmt 通过；定时任务 CRUD + 执行 + SSE 冒烟通过