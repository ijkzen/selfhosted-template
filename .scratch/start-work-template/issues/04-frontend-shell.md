# 04: 前端壳迁移

**What to build:** 把 llm-gateway 前端 web/ 的基础设施壳迁移为模板并统一命名：全部构建/测试配置（vite/tsc/vitest/biome/tailwind/shadcn）、App.tsx 路由分层（/login + RequireAuth + AppLayout + NotFound）、login/not-found 页、layout/nav（lib/pages.ts 改造）、require-auth、ky api client（lib/api.ts）、lib/utils、20 个 shadcn/ui 组件与通用零件（data-table 零件/confirm-dialog/page-header/error/empty-state/search-input/status-badge/segmented-control/theme/locale toggle/skip-to-main/scroll-to-top 等）、hooks（use-auth/theme/locale/toast/mobile/in-view/init-settings）、i18n 双语基础设施命名空间、主题切换。删除全部业务页/hook/组件/图表/业务命名空间/业务测试。数据表格零件、confirm-dialog、status-badge 中的业务硬编码文案移到通用命名空间；品牌与存储 key（theme/locale）统一为 selfhosted-template。

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] 配置与依赖迁移完成；业务文件全部删除
- [ ] 路由壳 + 导航 + 登录/404 + api client + 通用组件 + 基建 hooks 就绪，无业务引用
- [ ] i18n 只含基建命名空间；硬编码业务文案已通用化
- [ ] 命名/品牌/存储 key 统一
- [ ] 保留的组件测试全部通过；`pnpm build`（含 tsc 类型检查）/ `pnpm test` / Biome lint/format 通过