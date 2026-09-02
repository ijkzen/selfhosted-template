# 03: Notes 示例域后端

**What to build:** 新增最小通用示例域 Note，演示后端 CRUD 骨架：entity（id/title/content + 后端维护时间戳）、db 迁移建表、routes/notes（GET 列表 / POST 创建 / PUT 更新 / DELETE 删除，统一响应信封）、挂入路由树。为 Notes 全生命周期补 HTTP 黑盒集成测试（沿用 tests/common 基建）。

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] note 实体与迁移
- [ ] /api/notes CRUD 路由（信封、校验、404/错误处理）
- [ ] 集成测试：列表/创建/更新/删除 + 缺失/非法输入
- [ ] `cargo test --all-targets` / clippy / fmt 通过