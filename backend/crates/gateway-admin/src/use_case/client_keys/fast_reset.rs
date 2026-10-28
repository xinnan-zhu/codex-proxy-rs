//! Key Fast 计划来源，只消费当前账号范围和 Provider 本地周/月额度投影

use crate::{
    model::{
        AdminError, PageSize,
        accounts::{AccountListQuery, AccountPageItem},
        client_key_fast_reset::{ClientKeyFastResetPlan, FastResetObservation},
        provider_credentials::ProviderQuotaReadRequest,
    },
    ports::{
        provider::ProviderAdminRegistry,
        store::{AccountRuntimeStore, AccountStore},
    },
};
use chrono::Utc;
use gateway_core::account::AccountStatus;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct FastResetSource {
    pub accounts: Arc<dyn AccountStore>,
    pub runtime: Arc<dyn AccountRuntimeStore>,
    pub providers: ProviderAdminRegistry,
}

pub(crate) struct AccountObservations {
    pub(crate) account: AccountPageItem,
    pub(crate) observations: Vec<FastResetObservation>,
}

impl FastResetSource {
    pub async fn observations(&self) -> Result<Vec<AccountObservations>, AdminError> {
        let provider = gateway_core::identity::ProviderKind::new("openai")
            .map_err(|_| AdminError::internal("Provider 不合法"))?;
        let adapter = self
            .providers
            .require(&provider)
            .map_err(|_| AdminError::invalid("OpenAI 周/月额度不可用"))?;
        let runtime = self
            .runtime
            .active_rate_limits()
            .await
            .map_err(|_| AdminError::unavailable("账号运行状态不可用"))?;
        let mut result = Vec::new();
        let mut page = 1;
        loop {
            let rows = self
                .accounts
                .list_accounts(
                    AccountListQuery {
                        page,
                        page_size: PageSize::new(200)
                            .map_err(|_| AdminError::internal("页大小不合法"))?,
                        provider_kind: Some(provider.clone()),
                        group_filter: None,
                        search: None,
                        status: None,
                        sort: None,
                    },
                    runtime.clone(),
                )
                .await
                .map_err(|_| AdminError::unavailable("账号范围不可用"))?;
            let count = rows.items.len();
            let requests: Vec<_> = rows
                .items
                .iter()
                .map(|item| ProviderQuotaReadRequest {
                    account: &item.account,
                    rolling_usage: None,
                })
                .collect();
            let quotas = adapter.read_quotas(&requests).await;
            for (account, quota) in rows.items.into_iter().zip(quotas) {
                if let Ok(quota) = quota {
                    let observations = FastResetObservation::from_quota(&account.account, &quota);
                    result.push(AccountObservations {
                        account,
                        observations,
                    });
                }
            }
            if count < 200 {
                break;
            }
            if page >= 25 {
                return Err(AdminError::invalid("账号范围过大，自动恢复暂不可用"));
            }
            page += 1;
        }
        Ok(result)
    }

    pub async fn arm(
        &self,
        groups: &[gateway_core::routing::AccountGroupId],
        modes: &gateway_core::account::ModelFastModes,
    ) -> Result<ClientKeyFastResetPlan, AdminError> {
        let now = Utc::now();
        let baselines: Vec<_> = self
            .observations()
            .await?
            .into_iter()
            .filter(|item| {
                in_scope(item, groups, modes)
                    && item.account.projection.status == AccountStatus::Normal
            })
            .flat_map(|item| item.observations)
            .filter(|item| item.reset_at > now && item.observed_at <= now)
            .collect();
        let deadline = baselines
            .iter()
            .map(|item| item.reset_at)
            .min()
            .ok_or_else(|| {
                AdminError::invalid("可用账号缺少有效周/月额度重置时间，暂不能启用自动恢复")
            })?;
        Ok(ClientKeyFastResetPlan {
            generation: uuid::Uuid::now_v7().to_string(),
            armed_at: now,
            deadline,
            checked_at: now,
            completed_at: None,
            baselines,
        })
    }
}

pub(crate) fn in_scope(
    item: &AccountObservations,
    groups: &[gateway_core::routing::AccountGroupId],
    modes: &gateway_core::account::ModelFastModes,
) -> bool {
    let account = &item.account.account;
    account.enabled
        && !matches!(
            item.account.projection.status,
            AccountStatus::Disabled | AccountStatus::Error
        )
        && (groups.is_empty()
            || account
                .groups
                .iter()
                .any(|group| group.enabled && groups.contains(&group.id)))
        && modes
            .enabled_models(&account.provider_kind)
            .iter()
            .any(|model| account.model_access.allows(model))
}
