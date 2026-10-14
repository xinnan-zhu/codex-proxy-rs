//! 验证统计升级保留历史事实与回填进度，并重建已有时间桶

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{TEST_MIGRATOR, TestDatabase};

#[tokio::test]
async fn analytics_upgrade_preserves_facts_and_backfill_progress_and_requeues_buckets() {
    let Some(database) = TestDatabase::create_through("analytics_upgrade", 29).await else {
        return;
    };
    sqlx::raw_sql(
        "insert into request_analytics_facts(id,started_at,source_version) values
            ('first','2026-10-01 10:01:00+00',1),
            ('same_bucket','2026-10-01 10:02:00+00',2),
            ('next_bucket','2026-10-01 10:16:00+00',3);
         insert into request_analytics_pending_buckets(bucket_start,oldest_pending_at)
            values ('2026-10-01 10:00:00+00','2026-10-01 09:00:00+00');
         update request_analytics_build_state set
            backfill_complete=true, backfill_cursor_id='next_bucket',
            backfill_cursor_started_at='2026-10-01 10:16:00+00',
            coverage_start='2026-10-01 10:00:00+00',
            coverage_end='2026-10-01 10:30:00+00', generation=7;",
    )
    .execute(&database.pool)
    .await
    .unwrap();
    let facts_before: Vec<Value> =
        sqlx::query_scalar("select to_jsonb(f) from request_analytics_facts f order by id")
            .fetch_all(&database.pool)
            .await
            .unwrap();
    let progress_before: Value =
        sqlx::query_scalar("select to_jsonb(s) from request_analytics_build_state s")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    let pending_before: (i64, DateTime<Utc>) = sqlx::query_as(
        "select change_version,oldest_pending_at from request_analytics_pending_buckets",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();

    TEST_MIGRATOR.run(&database.pool).await.unwrap();

    let facts_after: Vec<Value> =
        sqlx::query_scalar("select to_jsonb(f) from request_analytics_facts f order by id")
            .fetch_all(&database.pool)
            .await
            .unwrap();
    let progress_after: Value =
        sqlx::query_scalar("select to_jsonb(s) from request_analytics_build_state s")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(facts_after, facts_before);
    assert_eq!(progress_after, progress_before);
    let pending: Vec<(DateTime<Utc>, i64, DateTime<Utc>)> = sqlx::query_as(
        "select bucket_start,change_version,oldest_pending_at
         from request_analytics_pending_buckets order by bucket_start",
    )
    .fetch_all(&database.pool)
    .await
    .unwrap();
    let expected_buckets = ["2026-10-01T10:00:00Z", "2026-10-01T10:15:00Z"]
        .map(|value| value.parse::<DateTime<Utc>>().unwrap());
    assert_eq!(
        pending.iter().map(|row| row.0).collect::<Vec<_>>(),
        expected_buckets
    );
    assert!(pending[0].1 > pending_before.0);
    assert_eq!(pending[0].2, pending_before.1);

    // 重启不会再次入队，也不会覆盖最早待处理时间
    TEST_MIGRATOR.run(&database.pool).await.unwrap();
    let pending_after_restart: Vec<(DateTime<Utc>, i64, DateTime<Utc>)> = sqlx::query_as(
        "select bucket_start,change_version,oldest_pending_at
         from request_analytics_pending_buckets order by bucket_start",
    )
    .fetch_all(&database.pool)
    .await
    .unwrap();
    assert_eq!(pending_after_restart, pending);
    database.close().await;
}

#[tokio::test]
async fn fork_throughput_upgrade_rebuilds_historical_grok_rates_without_rewriting_observations() {
    let Some(database) = TestDatabase::create_through("fork_throughput_upgrade", 30).await else {
        return;
    };
    // 官方统计迁移已执行，单独验证速率迁移能重建没有新请求的存量统计
    let previous = sqlx::migrate::Migrator::with_migrations(
        TEST_MIGRATOR
            .iter()
            .filter(|migration| {
                migration.version <= 30 || (900001..=900010).contains(&migration.version)
            })
            .cloned()
            .collect(),
    );
    previous.run(&database.pool).await.unwrap();
    let start = Utc::now() - chrono::TimeDelta::days(2);
    super::analytics::seed(&database.pool, "historical_grok_rate", start, &[]).await;
    sqlx::query("update model_requests set provider_kind='xai' where id='historical_grok_rate'")
        .execute(&database.pool)
        .await
        .unwrap();
    // 在旧表结构中直接还原已经完成的历史投影，升级前不运行新版 Worker
    sqlx::raw_sql("insert into request_analytics_facts(id,started_at,completed_at,client_api_key_ref,provider_kind,operation,client_transport,requested_model_id,outcome,client_status_code,downstream_committed_at,input_tokens,output_tokens,total_tokens,latency_ms,first_token_ms,cost_source,routing_group_refs,source_version) select id,started_at,completed_at,client_api_key_ref,provider_kind,operation,client_transport,requested_model_id,outcome,client_status_code,downstream_committed_at,input_tokens,output_tokens,total_tokens,latency_ms,first_token_ms,cost_source,routing_group_refs,nextval('request_analytics_change_version') from model_request_observations where id='historical_grok_rate'; delete from request_analytics_pending_requests; update request_analytics_build_state set backfill_complete=true,coverage_start=(select min(started_at) from request_analytics_facts),coverage_end=now();")
        .execute(&database.pool).await.unwrap();
    let observation_before: Value = sqlx::query_scalar(
        "select request_observation_json from model_requests where id='historical_grok_rate'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    let progress_before: Value = sqlx::query_scalar("select to_jsonb(state)-'generation'-'updated_at'-'coverage_end' from request_analytics_build_state state")
        .fetch_one(&database.pool).await.unwrap();
    let facts_before: Value = sqlx::query_scalar(
        "select to_jsonb(f) from request_analytics_facts f where id='historical_grok_rate'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    TEST_MIGRATOR.run(&database.pool).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from request_analytics_pending_buckets)"
        )
        .fetch_one(&database.pool)
        .await
        .unwrap()
    );
    let facts_after: Value = sqlx::query_scalar(
        "select to_jsonb(f) from request_analytics_facts f where id='historical_grok_rate'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(facts_after, facts_before);
    super::drain_analytics(&database.pool).await;
    let repo = super::observability_repository(&database.pool);
    let range = gateway_store::postgres::ObservabilityRange::new(
        start - chrono::TimeDelta::minutes(1),
        start + chrono::TimeDelta::hours(1),
    )
    .unwrap();
    let summary = repo.usage_summary(range, Default::default()).await.unwrap();
    assert!(
        summary
            .requests
            .output_throughput_p50
            .is_some_and(|value| (190..=210).contains(&value)),
        "historical output rate must remain within the histogram approximation tolerance of 200 tok/s"
    );
    assert_eq!(summary.requests.estimated_throughput_count, 1);
    let observation_after: Value = sqlx::query_scalar(
        "select request_observation_json from model_requests where id='historical_grok_rate'",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(observation_after, observation_before);
    let progress_after: Value = sqlx::query_scalar("select to_jsonb(state)-'generation'-'updated_at'-'coverage_end' from request_analytics_build_state state")
        .fetch_one(&database.pool).await.unwrap();
    assert_eq!(progress_after, progress_before);
    database.close().await;
}
