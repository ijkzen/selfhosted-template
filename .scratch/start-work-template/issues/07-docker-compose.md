# 07: Docker / Compose 适配

**What to build:** 保留并适配容器化文件到模板：根 Dockerfile 多阶段构建（阶段 1 node 构建 web/dist；阶段 2 rust 构建静态二进制，运行镜像只含二进制）、根 compose.yaml（服务名、卷、`SECRET_KEY`、`APP_ENV=prod` 路径约定）、.dockerignore、.env.example（按模板重写，`SECRET_KEY` 等）。不迁移 nightly/release/gitea/.deploy/deploy/scripts。

**Blocked by:** 01, 04

**Status:** ready-for-agent

- [ ] Dockerfile 多阶段构建通过（web/dist → cargo build → 精简运行镜像）
- [ ] compose.yaml / .dockerignore / .env.example 适配模板命名与 SECRET_KEY
- [ ] 本地 docker build 冒烟通过