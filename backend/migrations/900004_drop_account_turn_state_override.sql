-- 回退按账号覆盖 turn_state：应用层改回上游透传语义，删除该列。
alter table provider_accounts drop column turn_state_override;
