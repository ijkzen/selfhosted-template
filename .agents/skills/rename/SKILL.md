---
name: rename
description: 把本自托管后台模板（selfhosted-template）重命名为一个新项目：修改 Rust crate 名、web 包名、Docker 镜像/容器名、前端品牌名（i18n appName、页面标题、侧边栏文案）以及 localStorage key，并通过访谈生成新的 favicon 和 logo（SVG 转 PNG）。当用户想基于本模板开新项目、要求改项目名/包名/平台名/产品名，或说"rename / 重命名 / 换个名字"时使用。
---

# 重命名模板项目

把模板从 `selfhosted-template` 重命名为用户的新项目名。旧名有三种形式，都要替换：

- `selfhosted-template`（kebab-case）：项目名、镜像名、品牌文案
- `selfhosted_template`（snake_case）：Rust crate 名
- `selfhosted-template-web`：web 包名

## 第一步：向用户要名字

需要三种形式，缺哪种问哪种：

1. **项目名**（kebab-case）：用于目录、镜像、包名，如 `my-dashboard`
2. **crate 名**（snake_case）：通常由项目名机械转换 `my_dashboard`，不用单独问
3. **产品显示名**：UI 里给人看的名字，可以含空格和大写，如 `My Dashboard`

没被问到就自己机械转换，不要自创名字。

## 第二步：全量搜索旧名

先搜再改，确认清单没有遗漏（本 skill 写作时的清单见下方表格，仓库会演进，以搜索结果为准）：

```bash
grep -rn "selfhosted_template\|selfhosted-template" \
  --include="*.toml" --include="*.json" --include="*.ts" --include="*.tsx" \
  --include="*.rs" --include="*.yml" --include="*.yaml" --include="*.html" \
  . | grep -vE "node_modules|target/|\.codegraph|Cargo\.lock|pnpm-lock"
```

已知位置清单：

| 位置 | 内容 |
| --- | --- |
| `Cargo.toml` | `[package] name` 和 `[lib] name` |
| `src/main.rs`、`src/lib.rs`、`tests/` | `use selfhosted_template::...` 导入；`src/lib.rs` 里还有一行启动日志 `Starting selfhosted-template` |
| `Dockerfile` | 注释、`COPY .../target/release/selfhosted-template` 二进制路径、`CMD` |
| `compose.yaml` | 注释、service 名、`image:`、`container_name:` |
| `web/package.json` | `name`（`selfhosted-template-web`） |
| `web/index.html` | `<title>`、localStorage key `selfhosted-template-theme` |
| `web/src/i18n/index.ts` | `LOCALE_STORAGE_KEY`（`selfhosted-template-locale`） |
| `web/src/i18n/locales/{en,zh-CN}.ts` | `appName`、登录页 title |
| `web/src/components/layout.tsx`、`web/src/App.tsx` | 侧边栏 / 初始化页的品牌文案 |
| `web/src/hooks/use-theme.ts` | 注入样式 id 与 localStorage key `selfhosted-template-theme` |
| `web/src/test/setup.ts` | locale localStorage key |
| `web/src/__tests__/login-page.test.tsx` | 断言文案「登录 selfhosted-template」 |
| `web/public/favicon.svg`、`web/public/favicon-32x32.png` | 浏览器标签页图标（`web/index.html` 引用） |

## 第三步：替换

- 三种形式逐一替换；kebab ↔ snake 用机械转换，别改语义。
- 显示名（i18n `appName`、`<title>`、品牌文案）用用户给的产品名，不是 kebab-case 机器名。
- localStorage key（theme / locale）一并改。注意这会让老用户的主题和语言偏好重置一次，对全新项目无所谓，知道即可。
- `Cargo.lock` 不要手改，`cargo build` 后自动更新。

## 不要动的地方

- `docs/adr/0001-template-from-llm-gateway.md`、`CONTEXT.md`：记录模板出处（llm-gateway）的历史文档，保留原名。
- `.scratch/`：历史工作稿。
- `compose.yaml` 里的端口 `4007`、环境变量名、数据卷路径：与重命名无关，除非用户明确要求，否则不动。

## 第四步：生成 Favicon 与 Logo

现有资产只有两个，都在 `web/public/`：`favicon.svg`（矢量，`web/index.html` 引用）和 `favicon-32x32.png`（PNG 兜底）。UI 里的品牌标识是文字（`layout.tsx`），没有图片 logo。

**文件名保持不变，直接替换文件内容**，这样 `index.html` 不用改。

### 访谈

先问用户两轮问题，一次问完，不要挤牙膏：

1. **项目特点**：产品是做什么的？有没有想用的字母、符号或意象？品牌主色是什么（用户说不上来就从现有 UI 主题色里挑一个）？
2. **图标期望**：想要什么样子——字母标（首字母）还是抽象图形？几何极简还是有细节？圆角方形、圆形还是透明底？偏亮色还是深色？

### 生成 SVG

- 直接手写 SVG 文本（不要引外部库或图片），写入 `web/public/favicon.svg`。
- `viewBox` 用 `0 0 32 32`，以简单几何形状为主。
- favicon 在浏览器标签页实际显示只有 16px：避免细线条、渐变小字和复杂细节，缩小后糊成一团。
- 在浅色和深色背景下都要能认出来；默认透明底。
- 生成后让用户在浏览器里打开 SVG 确认效果，不满意就按反馈迭代，别自己拍板。

### 转 PNG

```bash
rsvg-convert -w 32 -h 32 web/public/favicon.svg -o web/public/favicon-32x32.png
```

`rsvg-convert` 不可用时，装一个再转（`brew install librsvg`），或用 ImageMagick 的 `magick` / `resvg`。

用户若额外要 UI 用的 logo 图片，同样先出 SVG，按需导出其他尺寸 PNG。

## 第五步：验证

改名会同时打断 Rust 和 web 两端的编译，两端都要跑：

```bash
cargo build && cargo test
cd web && pnpm lint && pnpm test run && pnpm build
```

测试断言里有品牌文案（如登录页标题），改了文案就要同步改断言。全部通过后再向用户报告完成。
