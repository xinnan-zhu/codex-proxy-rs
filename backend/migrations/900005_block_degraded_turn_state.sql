-- 全局开关：拒绝客户端头 x-codex-turn-state 恰好 312 字节的对话。默认关闭。
alter table runtime_settings add column block_degraded_turn_state boolean not null default false;
