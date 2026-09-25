-- 系统更新（检查 Release 与下载更新包）使用的已保存代理；为空表示直连。删除代理后回到直连。
alter table runtime_settings
    add column system_update_proxy_id text references outbound_proxies (id) on delete set null;
