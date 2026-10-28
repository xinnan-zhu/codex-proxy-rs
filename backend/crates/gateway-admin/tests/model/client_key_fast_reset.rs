//! 验证一次性 Fast 恢复的观测证据与跨重启确认

fn reset_observation(
    used: Option<u64>,
    revision: u64,
    observed: i64,
    reset: i64,
) -> gateway_admin::model::client_key_fast_reset::FastResetObservation {
    use chrono::{TimeZone as _, Utc};
    gateway_admin::model::client_key_fast_reset::FastResetObservation {
        account_id: "account-observation".into(),
        credential_revision: gateway_admin::model::Revision::new(revision).unwrap(),
        window_key: "weekly".into(),
        reset_at: Utc.timestamp_opt(reset, 0).unwrap(),
        observed_at: Utc.timestamp_opt(observed, 0).unwrap(),
        used_micros: used,
        zero_pending: false,
    }
}

fn reset_plan() -> gateway_admin::model::client_key_fast_reset::ClientKeyFastResetPlan {
    use chrono::{TimeZone as _, Utc};
    gateway_admin::model::client_key_fast_reset::ClientKeyFastResetPlan {
        generation: "test-generation".into(),
        armed_at: Utc.timestamp_opt(100, 0).unwrap(),
        deadline: Utc.timestamp_opt(1000, 0).unwrap(),
        checked_at: Utc.timestamp_opt(100, 0).unwrap(),
        completed_at: None,
        baselines: vec![reset_observation(Some(60_000_000), 1, 100, 1000)],
    }
}

#[test]
fn fast_reset_requires_new_compound_weekly_evidence_and_keeps_deadline() {
    use chrono::{TimeZone as _, Utc};
    let now = Utc.timestamp_opt(200, 0).unwrap();
    for observation in [
        reset_observation(Some(0), 1, 100, 2000), // 同一观测不能复用
        reset_observation(Some(60_000_000), 1, 150, 2000), // 时间漂移
        reset_observation(Some(50_000_000), 1, 150, 1000), // 单次比例下降
        reset_observation(None, 1, 150, 2000),    // 未知用量
        reset_observation(Some(0), 2, 150, 2000), // 新凭证只重建基线
        reset_observation(Some(0), 1, 201, 2000), // 未来观测不成立
    ] {
        let mut plan = reset_plan();
        assert!(!plan.observe(&[observation], now));
        assert_eq!(plan.deadline, Utc.timestamp_opt(1000, 0).unwrap());
    }
    let mut plan = reset_plan();
    assert!(plan.observe(&[reset_observation(Some(2_000_000), 1, 150, 2000)], now));
    assert!(reset_plan().observe(&[], Utc.timestamp_opt(1000, 0).unwrap()));
}

#[test]
fn fast_reset_confirmation_survives_restart_and_credential_refresh_rebases() {
    use chrono::{TimeZone as _, Utc};
    let now = Utc.timestamp_opt(200, 0).unwrap();
    let mut plan = reset_plan();
    assert!(!plan.observe(&[reset_observation(Some(0), 1, 150, 1000)], now));
    let mut restarted: gateway_admin::model::client_key_fast_reset::ClientKeyFastResetPlan =
        serde_json::from_value(serde_json::to_value(&plan).unwrap()).unwrap();
    assert_eq!(plan, restarted);
    assert!(!restarted.observe(&[reset_observation(Some(0), 1, 150, 1000)], now));
    assert!(restarted.observe(&[reset_observation(Some(0), 1, 160, 1000)], now));
    let mut refreshed = reset_plan();
    assert!(!refreshed.observe(&[reset_observation(Some(30_000_000), 2, 150, 1100)], now));
    assert!(refreshed.observe(&[reset_observation(Some(0), 2, 160, 2100)], now));
    let mut removed = reset_plan();
    assert!(!removed.observe(&[], now));
    assert!(!removed.baselines[0].zero_pending);
}
