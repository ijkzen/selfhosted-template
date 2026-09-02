# 0001: 模板化抽取自 llm-gateway

selfhosted-template 不是从零搭的脚手架，而是把 llm-gateway 的全部业务逻辑（供应商、虚拟模型、API Key、转发、用量、数据面板等）抽离，仅保留久经验证的前后端基础设施（axum 装配、SQLite 迁移框架、登录会话、cron 引擎、React 管理后台壳）后形成的通用单用户自托管后台模板。

**理由**：llm-gateway 的骨架在多轮迭代与生产使用中验证过（登录/设置/cron/前后端 CRUD 惯例），新项目以此起步可省去重写页面与接线。**被拒绝的替代方案**：从零搭建（丢失验证过的骨架）、保留业务域作示例（模板被 llm-gateway 语义污染）。

**Consequences**：已删除的业务逻辑不可回迁——需要时以 llm-gateway 仓库为参考重新实现；模板的通用能力（cron 引擎、`SECRET_KEY` 加密、双语 i18n）按通用语义命名，不保留 llm-gateway 专用命名（如 `API_KEY_ENCRYPTION_KEY`）。