-- 速率增加网关估算样本，保留历史事实与回填进度，仅重建统计时间桶
insert into request_analytics_pending_buckets(bucket_start, oldest_pending_at)
select distinct date_bin(interval '15 minutes', started_at, timestamptz '2000-01-01 UTC'), now()
from request_analytics_facts
on conflict (bucket_start) do update
set change_version = nextval('request_analytics_change_version');
