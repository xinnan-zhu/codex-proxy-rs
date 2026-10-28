//! Key Fast 的一次性恢复计划，只消费 Provider 已解释的周/月额度观测

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Revision, accounts::AccountRecord, provider_credentials::ProviderQuota};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientKeyFastResetPlan {
    pub generation: String,
    pub armed_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
    pub checked_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub baselines: Vec<FastResetObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FastResetObservation {
    pub account_id: String,
    pub credential_revision: Revision,
    pub window_key: String,
    pub reset_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    /// 百万分之一百分点，避免浮点序列化破坏 CAS 等值
    pub used_micros: Option<u64>,
    pub zero_pending: bool,
}

impl FastResetObservation {
    pub fn from_quota(account: &AccountRecord, quota: &ProviderQuota) -> Vec<Self> {
        let Some(observed_at) = quota.observed_at else {
            return Vec::new();
        };
        quota
            .usage_windows()
            .filter_map(|(window, _)| {
                Some(Self {
                    account_id: account.id.clone(),
                    credential_revision: account.credential_revision,
                    window_key: format!(
                        "{}:{}:{}",
                        window.group, window.key, window.window_seconds?
                    ),
                    reset_at: window.reset_at?,
                    observed_at,
                    used_micros: window
                        .used_percent
                        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 100.0)
                        .map(|v| (v * 1_000_000.0).round() as u64),
                    zero_pending: false,
                })
            })
            .collect()
    }
}

impl ClientKeyFastResetPlan {
    #[must_use]
    pub fn expired(&self, now: DateTime<Utc>) -> bool {
        self.completed_at.is_some() || self.deadline <= now
    }

    /// 新凭证只重建基线；同一凭证必须有严格更新的复合证据
    pub fn observe(&mut self, current: &[FastResetObservation], now: DateTime<Utc>) -> bool {
        self.checked_at = now;
        if self.expired(now) {
            return true;
        }
        for old in &mut self.baselines {
            let Some(new) = current.iter().find(|item| {
                item.account_id == old.account_id && item.window_key == old.window_key
            }) else {
                continue;
            };
            if new.observed_at <= old.observed_at || new.observed_at > now {
                continue;
            }
            if old.credential_revision != new.credential_revision {
                *old = new.clone();
                continue;
            }
            if let (Some(before), Some(after)) = (old.used_micros, new.used_micros) {
                if new.reset_at > old.reset_at && after < 10_000_000 && after < before {
                    return true;
                }
                if new.reset_at == old.reset_at && after == 0 && (before > 0 || old.zero_pending) {
                    if old.zero_pending {
                        return true;
                    }
                    let mut pending = new.clone();
                    pending.zero_pending = true;
                    *old = pending;
                    continue;
                }
            }
            // 每次更新以最近事实为基准，单次漂移或比例下降不会累计成重置证明
            *old = new.clone();
        }
        false
    }
}
