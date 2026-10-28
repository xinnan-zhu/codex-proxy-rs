-- Client Key 的 Fast 覆盖按 Provider 与实际上游模型保存
alter table client_api_keys
    add column model_fast_modes_json jsonb not null default '{}'::jsonb
    check (jsonb_typeof(model_fast_modes_json) = 'object');

-- 空值表示未启用，计划与确认基线随重启保留
alter table client_api_keys
    add column fast_mode_reset_json jsonb
    check (fast_mode_reset_json is null or jsonb_typeof(fast_mode_reset_json) = 'object');
create index client_api_keys_fast_mode_reset_pending_idx
    on client_api_keys ((fast_mode_reset_json->>'checkedAt'), id)
    where fast_mode_reset_json is not null and fast_mode_reset_json->>'completedAt' is null;
