//! 验证 Key 模型 Fast 规则的边界、精确匹配与继承

use gateway_core::{
    account::{FastMode, ModelFastModes},
    identity::ProviderKind,
};
use serde_json::json;

#[test]
fn model_rules_match_exact_provider_and_upstream_model_and_default_removes_override() {
    let rules: ModelFastModes = serde_json::from_value(json!({
        "openai": {"model-a":"enabled", "model-b":"disabled", "model-c":"default"}
    }))
    .unwrap();
    let openai = ProviderKind::new("openai").unwrap();
    let other = ProviderKind::new("other").unwrap();
    assert_eq!(
        rules.resolve(&openai, "model-a", FastMode::Disabled),
        FastMode::Enabled
    );
    assert_eq!(
        rules.resolve(&openai, "model-b", FastMode::Enabled),
        FastMode::Disabled
    );
    for id in ["model-c", "MODEL-A", "alias", "missing"] {
        assert_eq!(
            rules.resolve(&openai, id, FastMode::Disabled),
            FastMode::Disabled
        );
    }
    assert_eq!(
        rules.resolve(&other, "model-a", FastMode::Disabled),
        FastMode::Disabled
    );
    assert_eq!(
        serde_json::to_value(rules).unwrap(),
        json!({"openai":{"model-a":"enabled","model-b":"disabled"}})
    );
}

#[test]
fn invalid_and_unbounded_rules_are_rejected() {
    for value in [
        json!({"openai":{"":"enabled"}}),
        json!({"openai":{" model":"enabled"}}),
        json!({"openai":{"model*":"enabled"}}),
        json!({"openai":{"model\n":"enabled"}}),
        json!({"openai":{"model":"invalid"}}),
        json!({"openai":{"model":null}}),
        json!({"openai":{("x".repeat(257)):"enabled"}}),
        json!({"openai":(0..257).map(|id| (format!("model-{id}"), "enabled")).collect::<std::collections::BTreeMap<_,_>>()}),
    ] {
        assert!(serde_json::from_value::<ModelFastModes>(value).is_err());
    }
}

#[test]
fn automatic_restore_preserves_disabled_and_frozen_attempt_policy() {
    use gateway_core::account::scope::{
        ClientRoutingScope, FrozenAccountScope, RuntimeAccountDirectory,
    };
    use std::{
        collections::BTreeMap,
        sync::Arc,
        time::{Duration, SystemTime},
    };
    let rules: ModelFastModes =
        serde_json::from_value(json!({"openai":{"a":"enabled","b":"disabled"}})).unwrap();
    let provider = ProviderKind::new("openai").unwrap();
    let deadline = SystemTime::UNIX_EPOCH + Duration::from_secs(200);
    let scope = FrozenAccountScope::new(
        Arc::new(RuntimeAccountDirectory::new(BTreeMap::new())),
        ClientRoutingScope::AllAccounts,
    )
    .with_model_fast_modes(rules.clone())
    .with_model_fast_reset_at(Some(deadline));
    let prior = scope.freeze_model_fast_modes(deadline - Duration::from_secs(1));
    let after = scope.freeze_model_fast_modes(deadline);
    assert_eq!(
        prior
            .model_fast_modes()
            .resolve(&provider, "a", FastMode::Disabled),
        FastMode::Enabled
    );
    assert_eq!(
        after
            .model_fast_modes()
            .resolve(&provider, "a", FastMode::Disabled),
        FastMode::Disabled
    );
    assert_eq!(
        after
            .model_fast_modes()
            .resolve(&provider, "a", FastMode::Enabled),
        FastMode::Enabled
    );
    assert_eq!(
        after
            .model_fast_modes()
            .resolve(&provider, "b", FastMode::Enabled),
        FastMode::Disabled
    );
    assert_eq!(
        prior
            .freeze_model_fast_modes(deadline + Duration::from_secs(1))
            .model_fast_modes(),
        &rules
    );
}
