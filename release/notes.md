# v3.52.0

同步上游 v3.46.0 至 v3.52.0，保留按 Key 和模型设置 Fast、额度重置后恢复继承、xAI 额度刷新与推理强度记录、费用效率、速率估算和系统更新代理

## 新增功能

- 请求记录支持按历史账号名称或邮箱筛选，区分 Fast 与 Ultrafast 标识，内置 GPT-6.2 Astra 和 Sol 价目，Ultrafast 暂按常规价估算
- 额度调度支持独立配置 5 小时、周、月重置窗口系数，智能调度与额度调度分别保存偏好
- OpenAI 通用额度桶可配置重置后预热，默认关闭；定时预热支持多条 Cron 规则
- 请求历史保留期可设置为超过 31 天的正整数，保留已有设置
- 管理端支持安装为 PWA 独立窗口应用，源码构建可配置 URL 子路径前缀
- 插件可查询已保留的历史额度观测，调度插件可识别账号的会话绑定与选号关系

## 问题修复

- 改进 WebSocket 大请求、背压与建连失败恢复，同一请求跳过已确认不可达的代理入口
- 修正原生续接的容量拒绝恢复、Claude Messages 会话亲和及无状态码的流内请求错误分类
- 目录刷新、预热和连接测试使用全局客户端画像，自动画像每小时检查版本，保留模型排除规则
- 修复只读目录更新或回滚的权限处理，以及管理端主题和 PWA 标题栏显示
- 更新 Quinn、rustix 等依赖，修复已知安全问题

## 升级须知

- 出站请求未显式选择代理时，按部署的 HTTP_PROXY、HTTPS_PROXY、ALL_PROXY 和 NO_PROXY 设置选择出口；系统更新单独选择的代理继续优先使用
- 额度重置窗口系数默认全为 0；依赖重置时间评分的智能调度请至少设置一个正系数
- 管理 API 调用方须将请求记录参数 accountName 改为 accountSearch，将账号 quotaLimits 改为 quotaSettings，并为运行设置更新补充 quotaResetWindowWeights
- 调度插件 scheduler 能力升级至 v4，使用 v3 的插件须适配后使用
- 请使用 PostgreSQL 16 或更高版本，默认 Compose 使用 PostgreSQL 18；数据库迁移保留已有额度上限，额度桶预热默认关闭
- Responses 的部分会话与线程标识映射会变化，账号隔离保留；URL 子路径在构建时确定，使用此前缀需重新构建并配置反向代理
- 提供 Linux amd64 更新包和 [GHCR 镜像](https://github.com/xinnan-zhu/codex-proxy-rs/pkgs/container/codex-proxy-rs)

[上游变更对比](https://github.com/zyycn/codex-proxy-rs/compare/v3.45.2...v3.52.0)
