-- 账号级 Codex 客户端限制：true 时仅 Codex 官方客户端可调度到该账号。
alter table provider_accounts add column codex_only boolean not null default false;
