# 0002: 飞书通知自研 HTTP 客户端，触发点挂定时任务失败

## Status

accepted

## Context

模板需要一个「出站告警」能力：单用户自托管后台长期无人值守，出问题时必须能把消息推到人手上。llm-gateway 已有一套久经验证的飞书通知实现（渠道配置加密落库、扫码一键创建应用、测试发送、真实状态转换时异步通知），本模板按 ADR-0001 的沉淀原则把它整体移植过来。

移植时需要回答两个问题：**客户端怎么实现**、**触发点放在哪**。

## Decision

### 客户端自研 HTTP，不引入第三方飞书 SDK

能力面很小：三组官方端点共 6 个调用点——`POST /open-apis/auth/v3/tenant_access_token/internal`、`POST /open-apis/im/v1/messages?receive_id_type=…`、`accounts.feishu.cn/oauth/v1/app/registration`（`action=begin` / `action=poll`，OAuth 2.0 Device Authorization Grant）。社区 SDK（`lark-oapi` / `oapi-sdk-rust`）默认 features 编译不过且未发布 crates.io，用一个编译不过的 pre-alpha 包换 6 个调用点的薄封装不划算；`reqwest` 直调不引入额外抽象层。

只支持飞书国内版（`open.feishu.cn` / `accounts.feishu.cn`），不做 Lark 国际版域名切换。

### 触发点：定时任务执行失败

llm-gateway 的触发点是供应商可用性的自动转换，属业务专属，不搬。模板里业务无关的「后台自动干活」只有一处——定时任务引擎，因此触发点收敛在 `cron::worker::execute_with_logging` 的单次执行收尾：run 落成 `failed` 后异步发一条含任务名与失败原因的消息。

新项目接入自己的业务事件时，照 `src/notification/notify.rs` 加一个 `render_*` + `spawn_*` 即可，不需要改渠道层。

## Consequences

- **发送路径绝不阻塞主流程**：`spawn_failure` 是同步函数，内部 `tokio::spawn`，调用点在任务收尾路径上；未配置 / 已停用时静默返回（不构成错误）。
- **凭据加密落库**：`notification_channel.config` 整段 AES-256-GCM 加密（`enc:v1:` 前缀，密钥来自 `SECRET_KEY`）；接口对外只返回掩码后的 appSecret，明文不出后端。未配置 `SECRET_KEY` 时退化为明文存储并 warn，与其他敏感字段口径一致。
- **编辑不必重输凭据**：保存时 `appSecret` 缺省/空串沿用库中原值；首次创建必须提供。
- **扫码注册的轮询状态机由我们自己维护**，三个易错点必须守住：HTTP 400 的响应体也要解析（`authorization_pending` / `slow_down` / `access_denied` / `expired_token` 均以 400 返回）、`slow_down` 时轮询间隔 +5s、`expires_in` 与 `expire_in` 两种字段名都接受。会话是**单槽位内存态**，重启即丢。
- **`tenant_access_token` 不做进程内缓存**，每次发送现取（官方语义：剩余有效期 >30 分钟时返回同一 token）。
- **集成测试用本地 mock 飞书**，经 `FEISHU_BASE_URL` 环境变量重定向基址；该变量仅供测试，生产不设置。
- 术语见 `CONTEXT.md`「通知渠道」「扫码创建」「任务失败通知」。
