# Fork 差异说明（vs 上游 zyycn/codex-proxy-rs）

本文档记录本 fork 相对官方仓库的全部差异，供维护与升级参考。

- 上游基线：`main`（已合并至 v3.13.0）
- Fork 分支：`feat/account-turn-state-override`
- 部署实例：`api2.koalaccc.xyz`（compose 位于 `/root/codex-proxy-rs/deploy/`，镜像 `cpr-local:turnstate-*` 系列）

## 功能总览

| # | 功能 | 提交 | 上游是否有 |
|---|------|------|-----------|
| 1 | 账号级「仅限 Codex 官方客户端」开关 | `445215d3` `79673e5a` | 无（模仿 sub2api `codex_cli_only`） |
| 2 | 请求明细统计 turn-state 字节数 | `9e51d04e` | 无 |
| 3 | 使用统计列表：密钥列 + 智商列 | `028eab7c` `fbd1816b` `0d1b0e9e`（部分回退） | 密钥名称列已由上游 v3.12.0 提供；智商列为 fork 保留 |
| — | 合并上游 v3.10.0 | `00be1c6f` | — |
| — | 回退按账号覆盖 X-Codex-Turn-State | `e8d6ec5e` `33097947` `7a138de7` 已从应用层移除；补偿迁移 `900004` | 上游本无此功能 |
| — | 合并上游 v3.11.0 | `cdf9df18` | Client Key 额度重置改用官方实现（`period`: daily/weekly/all） |
| — | 合并上游 v3.12.1 | `41766143` | 官方用量列表增加密钥名称与长上下文计费标记；fork 保留智商列 |
| — | 合并上游 v3.13.0 | 本次 | 官方用量列表在账号下展示备注；无新迁移 |

---

## 1. 账号级「仅限 Codex 官方客户端」开关（codex_only）

**动机**：模仿 sub2api 的 `codex_cli_only`，防止非 Codex 客户端的流量打到 Codex 账号上（避免上游风控）。

**语义**：`provider_accounts.codex_only` 布尔，默认 `false`。开启后，非官方 Codex 客户端的请求在**调度选号阶段**即被排除；无可服务账号时返回 403：

```
{"error": {"code": "policy_denied", "message": "This account only allows Codex official clients"}}
```

管理端「连接测试」诊断路径豁免（`is_diagnostic_required_account`），xAI provider 不消费该字段。

**官方客户端识别**（`gateway-api/src/openai/auth.rs` 的 `official_codex_client`）：镜像 sub2api 名单——

- 9 个 UA 严格前缀：`codex_cli_rs/`、`codex-tui/`、`codex_vscode/`、`codex_vscode_copilot/`、`codex_app/`、`codex_chatgpt_desktop/`、`codex_atlas/`、`codex_exec/`、`codex_sdk_ts/`
- `Codex ` 家族前缀（如 `Codex Desktop/…`）
- originator 精确集（同上 9 个名字）
- UA 尾部括号组兜底（前缀被 `CODEX_INTERNAL_ORIGINATOR_OVERRIDE` 改写时，从 `(name; version)` 恢复真实身份）

**与最低版本策略解耦**：`identify_codex_client`（原有，只管 Desktop/CLI 最低版本）保持不变；门控用独立的 `official_codex_client`。修复提交 `79673e5a` 前曾直接复用 `identify_codex_client`，导致 `codex_exec`、`codex-tui` 等官方家族被误拒。

**传导链**：边缘识别 → `ResponsesRequestMetadata.codex_client` → `ExecutionRequestMetadata` → `NewModelRequest` → coordinator → `RequestAttemptContext`/`AttemptContext.codex_client()` → openai selector 过滤（含 trace 计数）。新 ProviderErrorKind `AccountClientRestricted` → `GatewayErrorKind::PolicyDenied`（403）。

**与 sub2api 的对齐情况**：开关粒度、识别名单、拒绝文案、诊断豁免均已对齐；sub2api 全局设置里的高级门（黑/白名单、版本上下限、引擎指纹、App Server 开闸、全局 force 旁路）**未搬**。

## 2. 请求明细统计 turn-state 字节数

**动机**：观察对话 `X-Codex-Turn-State` 随轮次的增长（实际观察：292B = 满血模型，312B = 降智）。

**实现**：

- 迁移 `900003_request_turn_state_bytes.sql`：`model_requests` 加可空列 `client_turn_state_bytes int4`。
- **仅统计请求头** `x-codex-turn-state`（用户明确要求；body `turnState` / WS client_metadata 载体不计，注释与 docs/api.md 已注明）。没带该头记 NULL（区别于 0 字节）；统计客户端原始值。
- 测量点在 HTTP/WS 共用的请求解码层（`OpenAiRequestHeaders.turn_state` 的 `len()`）。
- 全链路：metadata → `NewModelRequest` → 两个 insert 点 → store 读取 → admin 两个视图（`clientTurnStateBytes`）→ 前端详情「客户端与上游」区「Turn State」行（<1024 显示 `B`，否则一位小数 `KB`）。

## 3. 使用统计列表：密钥列 + 智商列

**动机**：不用点开详情即可看到每请求的发起 key 与满血/降智状态。

**实现**：

- **密钥列**：v3.12.0 起采用官方 `clientApiKeyName`（「密钥名称」），并保留官方「账号」列。列表仍 `left join client_api_keys`；key 已删时名称为空。
- **「智商」列**（模型列右侧，`028eab7c` 新增、`fbd1816b` 改显示）：`clientTurnStateBytes` 映射——`292` 显示「满血」、`312` 显示「降智」，其它未知大小回退原始字节数（便于发现新档位），无值显示 —。
- **回退（`0d1b0e9e`）**：`4ec11cdb` 曾按错误理解改详情弹窗（标题追加 `· Turn State …`、首行「账号」改「密钥」），已还原——详情弹窗保持原标题「使用记录详情」与「账号」行；详情内「Turn State」原始字节数行保留。

---

## 维护说明

- **合并上游**：`git fetch upstream && git merge upstream/main`；若上游新增迁移号与 90000N 撞号，重编号本 fork 迁移并同步 `_sqlx_migrations` 表与 `.frozen-sha256`。v3.13.0 仍止于 `0016`，与 90000N 不冲突。
- **已回退的 turn_state 覆盖**：`900001` 已冻结，不可删改；`900004` 删除 `provider_accounts.turn_state_override`。已部署实例升级后该列消失，发往上游的 `X-Codex-Turn-State` 恢复与官方一致的透传。
- **质量门惯例**：每次变更跑 `cargo fmt/check/clippy（-D warnings）` + `pnpm format:check/build`；按用户要求**不跑单测**（测试代码仅补构造点保持可编译）。
- **部署**：`docker build --target runtime -f deploy/Dockerfile -t cpr-local:<tag> <src>`（必须 `--target runtime`；编译期内存紧张时先加 2G swapfile）；compose 位于 `/root/codex-proxy-rs/deploy/compose.yaml`（`CPR_IMAGE` 切换镜像，内存上限 1100m）。旧镜像保留作回滚。
- **GitHub**：`https://github.com/xinnan-zhu/codex-proxy-rs`（`origin`）；官方上游为 `upstream`（`zyycn/codex-proxy-rs`）。合并官方更新：`git fetch upstream && git merge upstream/main`，再 `git push origin`。
