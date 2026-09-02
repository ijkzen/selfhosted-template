# 05: Notes 前端页面

**What to build:** 新增 Notes 示例页面，演示前端 CRUD 骨架：列表（复用现有表格零件与分页/列显隐）、新建/编辑弹窗（react-hook-form + zod）、删除确认（confirm-dialog），挂入导航与路由，接入 /api/notes。补页面/表单/弹窗的组件级测试（vitest + testing-library）。

**Blocked by:** 03, 04

**Status:** ready-for-agent

- [ ] Notes 页面 + 路由 + 导航项，i18n 文案（示例命名空间）
- [ ] 列表/新建/编辑/删除交互与 /api/notes 联通
- [ ] 页面与表单测试通过
- [ ] `pnpm build` / `pnpm test` / Biome 通过；前后端联调冒烟（登录后增删改查一条 Note）