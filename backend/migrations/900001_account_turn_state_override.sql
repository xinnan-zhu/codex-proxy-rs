-- 按账号覆盖上游 turn state：NULL 不覆盖，非空强制为该值，空字符串剥离。
alter table provider_accounts
    add column turn_state_override text
    check (turn_state_override is null or char_length(turn_state_override) <= 2048);
