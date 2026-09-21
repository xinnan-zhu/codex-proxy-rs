# 本 fork 相对官方改了什么

本仓库是 [zyycn/codex-proxy-rs](https://github.com/zyycn/codex-proxy-rs) 的个人维护 fork，不是官方仓库。

- 当前基线：官方 `main` **v3.13.0**
- 远程：`origin` 为本仓库，`upstream` 为官方仓库

官方功能、部署方式和客户端接入仍以官方 README / 文档为准。这里只记录本 fork **多出来的**、以及**明确不再保留**的差异。

## 多出来的功能

### 1. 账号级「仅限 Codex 官方客户端」

管理端账号可打开 `codex_only`（默认关）。打开后，该 Codex 账号在调度选号时只接受官方 Codex 客户端；其它客户端不会打到这张号上。

无可服务账号时返回 403：

```json
{"error": {"code": "policy_denied", "message": "This account only allows Codex official clients"}}
```

识别名单对齐 sub2api 的官方 Codex 家族（CLI / TUI / VS Code / Desktop / Exec / SDK 等）。管理端「连接测试」走诊断路径，不受此开关拦截。xAI 账号不使用该字段。

未搬 sub2api 的全局黑白名单、版本上下限、引擎指纹等高级门。

实现要点：迁移 `900002`；边缘用独立的 `official_codex_client` 识别，不和官方「最低版本」逻辑混用。

### 2. 请求明细统计 Turn State 字节数

每条请求记录客户端请求头 `x-codex-turn-state` 的字节数（没带该头为 `null`，与 0 字节区分）。只统计请求头，不统计 body 里的 `turnState`。

详情里「客户端与上游」区有「Turn State」行：小于 1024 显示 `B`，否则一位小数 `KB`。

实际观察：`292` 字节对应满血模型，`312` 字节对应降智。

实现要点：迁移 `900003`，列 `model_requests.client_turn_state_bytes`。

### 3. 使用统计列表的「智商」列

使用记录列表在模型列右侧增加「智商」列，样式与「接入」列相同的圆角数字徽章：

| 字节数 | 显示 |
|--------|------|
| 292 | 绿色徽章，数字 `292` |
| 312 | 红色徽章，数字 `312` |
| 其它已知大小 | 中性徽章，显示原始数字 |
| 无值 | — |

密钥名称列用官方 v3.12.0 起的 `clientApiKeyName`（「密钥名称」），账号列也保留。详情弹窗标题和「账号」行与官方一致；详情内仍显示 Turn State 原始字节数。

## 明确不再保留

- **按账号覆盖 `X-Codex-Turn-State`**：曾经加过，已从应用层回退。发往上游的该头与官方一样透传。迁移 `900001` 冻结保留，`900004` 删列。
- **Client Key 额度重置**：v3.11.0 起用官方实现（`period`: daily / weekly / all）。不要恢复本 fork 旧的「只清零、重置窗口锚点」版本。

## 数据库迁移

本 fork 的定制迁移编号为 **90000N**，避免和官方 `0016` 及之后的编号冲突。官方 v3.13.0 仍止于 `0016`。

| 编号 | 作用 |
|------|------|
| `900001` | 曾加账号 turn_state 覆盖列（已冻结，不可改） |
| `900002` | `provider_accounts.codex_only` |
| `900003` | `model_requests.client_turn_state_bytes` |
| `900004` | 删除 turn_state 覆盖列 |

## 同步官方更新

```bash
git fetch upstream
git merge upstream/main
git push origin
```

冲突时：额度重置和官方 UI 改动优先上游；上面三项 fork 功能保留。若官方新迁移号与 `90000N` 撞号，重编号本 fork 迁移，并同步 `_sqlx_migrations` 与 `.frozen-sha256`。
