# 01: 后端核心骨架（无 cron）

**What to build:** 把 llm-gateway 的后端通用基础设施迁移进 selfhosted-template 并去掉全部业务接线，产出一个能启动、能完成"初始化建号 → 登录 → 设置"的最小可用后端骨架。具体保留：main/lib 装配（tracing、graceful shutdown、DB 连接）、config、response 统一信封、i18n、middleware、static_assets（rust-embed）、logs_cleanup、crypto（`SECRET_KEY` 泛化）、app_settings、entity{user,session,setting}、auth 完整登录（去掉 API Key Bearer 分支）、routes{auth,settings,healthz}、db 迁移框架（建表清单只含模板表）。不迁移 cron（见 #02）。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 依赖裁剪：移除转发栈/用量/签名等业务依赖，保留通用依赖
- [ ] 路由树含 auth/settings/healthz + 静态资源 fallback
- [ ] db 迁移只建 user/session/setting 等模板表，版本迁移重写
- [ ] auth 去掉 `/v1` Bearer 分支；SECRET_KEY 泛化
- [ ] tests/common 基建适配新 AppState；基础设施单测迁移并全绿（config/crypto/db/auth/logs_cleanup）
- [ ] `cargo test --all-targets` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo fmt --check` 通过；可启动并完成初始化登录（HTTP 冒烟）