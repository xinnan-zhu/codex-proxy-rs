-- Turn State 字节数不再用于阻断响应，移除对应运行配置。
alter table runtime_settings drop column block_degraded_turn_state;
