# v3.45.2

同步上游 v3.45.0 至 v3.45.2。保留分支按 Key 和模型设置 Fast、额度重置后恢复继承、xAI 额度刷新与推理强度记录、费用效率与速率估算，以及系统更新代理。

## 接入与账号调度

- 新增 Anthropic Messages 接入及输入 Token 计数。API Key 账号可选 Messages 上游，与 Responses、Chat Completions 客户端互通。
- OpenAI OAuth 账号可设定通用额度窗口的使用率上限；达到上限后暂停分配新请求，在途请求继续。可配置周额度偏低时提前使用重置卡。
- Codex 配置模板默认开启模型发现和实验性的异步上下文压缩。模型目录按密钥可用账号合并；Responses API Key 上游按 Codex 版本协商目录，级联 CPR 时保留上游声明的推理档位。
- 轮询仅在成功分配账号后推进。收到 `NO_MORE_RETRY` 时停止重试当前账号。原生 Responses 保留显式 `auto/default` 档位，并在出站时省略不兼容的输出 Token 上限。

## 记录、互联与安全

- 使用记录区分明确失败与结果未知；错误详情区分 HTTP 响应和 WebSocket 错误，并标记尝试记录可能缺失的情况。Messages 请求记录显式指定的推理强度。
- CPR 互联可共享所选分组中的 API Key 账号，并阻止请求回到已经过的节点。路由路径头仅发送给已识别的 CPR 节点。
- 更新 SQLite 和 OpenSSL 依赖，修复已知安全问题。

## 升级说明

- 从本分支 v3.44.0 升级会自动执行数据库迁移。旧定时预热计划会清空并关闭，已选模型保留；继续使用时请按部署时区填写五段 Cron 表达式并重新启用。
- 使用 CPR 互联时，请检查共享分组中的账号，并让节点间的反向代理保留 `X-CPR-Route`。
- 提供 Linux amd64 在线更新包和 [GHCR 镜像](https://github.com/xinnan-zhu/codex-proxy-rs/pkgs/container/codex-proxy-rs)。

[上游变更对比](https://github.com/zyycn/codex-proxy-rs/compare/v3.44.0...v3.45.2)
