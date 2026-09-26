# 本 fork 相对官方改了什么

本仓库是 [zyycn/codex-proxy-rs](https://github.com/zyycn/codex-proxy-rs) 的个人维护 fork，不是官方仓库。

- 当前基线：官方 `main` **v3.16.0**（含其后的更新进度布局修复）
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

### 4. 阻断降智对话

管理端「安全与访问」有全局开关 `block_degraded_turn_state`（默认关）。312 是上游响应里的 `x-codex-turn-state`，不是客户端先发明的。打开后，上游响应头或 WebSocket metadata 里的该值恰好 312 字节时，这次响应不发给客户端，改为英文 403。292、其它长度、没返回该头的响应照常交付。管理端「连接测试」走诊断路径，不受此开关拦截。

关闭时与未加此门时一致：上游返回的 312 仍转给客户端，使用记录智商列照常显示客户端回带的请求头长度。

不改写发往上游的 `X-Codex-Turn-State`，也不按账号跳过再换号。判定只看上游返回值的字节数，不读 body `turnState`。

拒绝为 HTTP 403，`error.code` 为 `policy_denied`，英文 message 固定为：

```text
This conversation triggered upstream degraded-intelligence risk control and was blocked.
```

实现要点：迁移 `900005`，列 `runtime_settings.block_degraded_turn_state`。

### 5. 账号额度的周期费用与重置进度

账号管理的额度面板和列表额度单元格，在每个额度窗口下显示：

- 本周期已用费用（按本地使用记录的 USD 计费）；
- 估算总额度：本周期费用 ÷ 已用百分比，已用比例过低时不估算；
- 蓝色重置进度条，与额度条等长并紧贴其下，标签行显示「距重置 X」，用于直观对比额度消耗与周期时间的比例。

实现要点：`quota_forecast::window_quota_usd_estimate`；账号额度窗口接口多了 `estimatedQuotaUsd`、`estimatedQuotaUsdDisplay`、`resetAt`，窗口本地用量多了 `billingAmountUsd`、`costEstimateStatus`。

### 6. 使用统计的模型筛选与总费用

- 使用统计页右上角可按模型筛选；选项只包含筛选范围内有成功调用的模型，错误请求里的模型名不出现。筛选同时作用于使用记录、洞察图表与错误面板。
- 顶部概览卡片增加「总费用」，副标题显示每次成功请求的平均费用；存在计费不完整的请求时改为提示实际可能更高。

### 7. 系统更新走代理

系统更新弹窗有「下载代理」下拉框：可选「直连」（默认）或代理管理里已保存的任一代理。选择会保存到服务端，检查 Release、下载更新包和 checksum 都走同一出口；更新日志的「获取 Release」一行会注明本次是直连还是经由哪个代理（不含认证信息）。删除被选中的代理后自动回到直连。

实现要点：迁移 `900006`，列 `runtime_settings.system_update_proxy_id`（外键 `outbound_proxies`，删除时置空）；接口 `GET/POST /api/admin/system/update/proxy`。

## 明确不再保留

- **按账号覆盖 `X-Codex-Turn-State`**：曾经加过，已从应用层回退。发往上游的该头与官方一样透传。迁移 `900001` 冻结保留，`900004` 删列。
- **Client Key 额度重置**：v3.11.0 起用官方实现（`period`: daily / weekly / all）。不要恢复本 fork 旧的「只清零、重置窗口锚点」版本。

## 数据库迁移

本 fork 的定制迁移编号为 **90000N**，避免和官方 `0016` 及之后的编号冲突。官方 v3.16.0 止于 `0019`；上游新迁移排在 `90000N` 之前也会按缺失补跑。

| 编号 | 作用 |
|------|------|
| `900001` | 曾加账号 turn_state 覆盖列（已冻结，不可改） |
| `900002` | `provider_accounts.codex_only` |
| `900003` | `model_requests.client_turn_state_bytes` |
| `900004` | 删除 turn_state 覆盖列 |
| `900005` | `runtime_settings.block_degraded_turn_state` |
| `900006` | `runtime_settings.system_update_proxy_id` |

## 预编译镜像

本机内存紧，不要在部署机上 `docker build`。GitHub Actions 在推送 `main` 或手动触发 `Publish image` 后，把运行时镜像推到 GHCR：

```text
ghcr.io/xinnan-zhu/codex-proxy-rs:latest
ghcr.io/xinnan-zhu/codex-proxy-rs:sha-<commit>
```

首次发布后把该 Package 设为 Public（GitHub → Packages → `codex-proxy-rs` → Package settings → Change visibility），然后本机无需登录即可拉取：

```bash
docker pull ghcr.io/xinnan-zhu/codex-proxy-rs:latest
cd /path/to/deploy
CPR_IMAGE=ghcr.io/xinnan-zhu/codex-proxy-rs:latest docker compose up -d --no-build --no-deps --force-recreate codex-proxy-rs
```

不要用官方 `ghcr.io/zyycn/codex-proxy-rs`，那份镜像没有本 fork 的改动。

管理端「检查更新」读的是 GitHub Releases（`CPR_UPDATE_REPOSITORY=xinnan-zhu/codex-proxy-rs`），不是 GHCR。点更新会下载：

```text
codex-proxy-rs_<version>_linux_amd64.tar.gz
checksums.txt
```

发布方式：推送 tag `vX.Y.Z`，或在 Actions 里手动运行 **Publish release**（可填版本；留空则用 `release/version.yaml`）。版本必须是官方更新器能识别的 SemVer：`3.13.1` 或 `3.13.0-exp.1` 这类。当前运行若是正式版 `3.13.0`，必须发**更高的正式版号**（例如 `3.13.1`）管理端才会提示可更新；同号或 `exp` 不会从正式版升上去。

不要改官方的 `release.yml`：它绑定官方仓库名和发版脚本，合入上游时会冲突。本 fork 只用 `publish-release.yml`。

## 同步官方更新

```bash
git fetch upstream
git merge upstream/main
git push origin
```

冲突时：额度重置和官方 UI 改动优先上游；上面各项 fork 功能保留。若官方新迁移号与 `90000N` 撞号，重编号本 fork 迁移，并同步 `_sqlx_migrations` 与 `.frozen-sha256`。
