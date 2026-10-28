//! Client API Key 管理用例

pub(crate) mod fast_reset;

use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_core::{
    engine::budget::ClientBudgetStatus, policy::ClientApiKeyId, runtime::SnapshotControl,
};
use rand_core::{OsRng, RngCore as _};
use uuid::Uuid;

use crate::{
    model::{
        AdminError, MutationContext,
        client_keys::{
            ClientKeyBudgetMutationOrigin, ClientKeyCursorValue, ClientKeyListQuery,
            ClientKeyMutation, ClientKeyPage, ClientKeyRecord, ClientKeySecret, ClientKeySortField,
            CreateClientKey, CreatedClientKey, DeleteClientKey, NewClientKey, ResetClientKeyBudget,
            SetClientKeyEnabled, UpdateClientKey, UpdateClientKeyBudgetLimits,
        },
    },
    ports::store::{AdminStoreError, AdminStoreErrorKind, ClientKeyStore},
};

use super::{map_store_error, publish_committed};

/// API 消费的 Client Key 管理服务
#[async_trait]
pub trait ClientKeyService: Send + Sync {
    fn fast_models(&self)
    -> Result<Vec<crate::model::client_keys::ClientKeyFastModel>, AdminError>;

    async fn get(&self, id: &ClientApiKeyId) -> Result<ClientKeyRecord, AdminError>;
    async fn list(&self, query: ClientKeyListQuery) -> Result<ClientKeyPage, AdminError>;
    async fn reveal(&self, id: &ClientApiKeyId) -> Result<ClientKeySecret, AdminError>;
    async fn create(
        &self,
        context: &MutationContext,
        command: CreateClientKey,
    ) -> Result<CreatedClientKey, AdminError>;
    async fn update(
        &self,
        context: &MutationContext,
        command: UpdateClientKey,
    ) -> Result<ClientKeyMutation, AdminError>;
    async fn set_enabled(
        &self,
        context: &MutationContext,
        command: SetClientKeyEnabled,
    ) -> Result<ClientKeyMutation, AdminError>;
    async fn delete(
        &self,
        context: &MutationContext,
        command: DeleteClientKey,
    ) -> Result<ClientKeyMutation, AdminError>;
    async fn budget(&self, id: &ClientApiKeyId) -> Result<ClientBudgetStatus, AdminError>;
    async fn update_budget_limits(
        &self,
        context: &MutationContext,
        command: UpdateClientKeyBudgetLimits,
        origin: ClientKeyBudgetMutationOrigin,
    ) -> Result<ClientApiKeyId, AdminError>;
    async fn reset_budget(
        &self,
        context: &MutationContext,
        command: ResetClientKeyBudget,
        origin: ClientKeyBudgetMutationOrigin,
    ) -> Result<ClientApiKeyId, AdminError>;
}

pub(crate) struct DefaultClientKeyService {
    providers: crate::ports::provider::ProviderAdminRegistry,
    store: Arc<dyn ClientKeyStore>,
    snapshot: Arc<dyn SnapshotControl>,
    published: Option<gateway_core::runtime::RuntimeSnapshotHandle>,
    fast_reset: Option<fast_reset::FastResetSource>,
}

impl DefaultClientKeyService {
    #[must_use]
    pub(crate) fn new(
        store: Arc<dyn ClientKeyStore>,
        snapshot: Arc<dyn SnapshotControl>,
        providers: crate::ports::provider::ProviderAdminRegistry,
        published: Option<gateway_core::runtime::RuntimeSnapshotHandle>,
        fast_reset: Option<fast_reset::FastResetSource>,
    ) -> Self {
        Self {
            store,
            snapshot,
            providers,
            published,
            fast_reset,
        }
    }
}

#[async_trait]
impl ClientKeyService for DefaultClientKeyService {
    fn fast_models(
        &self,
    ) -> Result<Vec<crate::model::client_keys::ClientKeyFastModel>, AdminError> {
        let snapshot = self
            .published
            .as_ref()
            .ok_or_else(|| AdminError::unavailable("模型目录不可用"))?
            .acquire()
            .map_err(|_| AdminError::unavailable("模型目录不可用"))?;
        // 当前 Fast 正文策略只由 OpenAI Responses 执行，其他 Provider 不展示可配置项
        let provider = gateway_core::identity::ProviderKind::new("openai")
            .map_err(|_| AdminError::internal("Provider 不合法"))?;
        Ok(snapshot
            .upstream_model_profiles_for_provider(&provider)
            .into_iter()
            .map(|profile| crate::model::client_keys::ClientKeyFastModel {
                provider: provider.clone(),
                model: profile.model().as_str().to_owned(),
                label: profile
                    .presentation()
                    .display_name()
                    .unwrap_or(profile.model().as_str())
                    .to_owned(),
                supports_priority: profile
                    .presentation()
                    .service_tiers()
                    .iter()
                    .any(|tier| tier.id() == "priority"),
            })
            .collect())
    }

    async fn get(&self, id: &ClientApiKeyId) -> Result<ClientKeyRecord, AdminError> {
        self.store
            .get_client_key(id)
            .await
            .map_err(|error| map_store_error(error, "client API key"))?
            .ok_or_else(|| AdminError::not_found("Client API Key 不存在"))
    }

    async fn budget(&self, id: &ClientApiKeyId) -> Result<ClientBudgetStatus, AdminError> {
        self.get(id).await.map(|key| key.budget)
    }

    async fn update_budget_limits(
        &self,
        context: &MutationContext,
        command: UpdateClientKeyBudgetLimits,
        origin: ClientKeyBudgetMutationOrigin,
    ) -> Result<ClientApiKeyId, AdminError> {
        if command.daily_limit_usd.is_none() && command.weekly_limit_usd.is_none() {
            return Err(AdminError::invalid("至少指定一个预算上限"));
        }
        let id = command.id.clone();
        if let Some(revision) = self
            .store
            .update_client_key_budget_limits(command, origin, context)
            .await
            .map_err(|error| map_store_error(error, "client API key"))?
        {
            publish_committed(self.snapshot.as_ref(), revision).await?;
        }
        Ok(id)
    }

    async fn reset_budget(
        &self,
        context: &MutationContext,
        command: ResetClientKeyBudget,
        origin: ClientKeyBudgetMutationOrigin,
    ) -> Result<ClientApiKeyId, AdminError> {
        let id = command.id.clone();
        self.store
            .reset_client_key_budget(command, origin, context)
            .await
            .map_err(|error| map_store_error(error, "client API key"))?;
        Ok(id)
    }

    async fn list(&self, query: ClientKeyListQuery) -> Result<ClientKeyPage, AdminError> {
        validate_cursor(&query)?;
        self.store
            .list_client_keys(query)
            .await
            .map_err(|error| map_store_error(error, "client API key"))
    }

    async fn reveal(&self, id: &ClientApiKeyId) -> Result<ClientKeySecret, AdminError> {
        self.store
            .reveal_client_key(id)
            .await
            .map_err(|error| map_store_error(error, "client API key"))?
            .ok_or_else(|| AdminError::not_found("Client API Key 不存在"))
    }

    async fn create(
        &self,
        context: &MutationContext,
        command: CreateClientKey,
    ) -> Result<CreatedClientKey, AdminError> {
        crate::model::client_keys::validate_group_ids(&command.group_ids)
            .map_err(|_| AdminError::invalid("Client API Key 分组不合法"))?;
        for (provider, profile) in &command.request_profile_overrides {
            self.providers
                .require(provider)
                .and_then(|provider| provider.preview_client_profile(profile))
                .map_err(|error| super::map_provider_error(error, "client profile"))?;
        }
        let fast_mode_reset = if command.reset_fast_mode_on_quota_reset {
            Some(
                self.fast_reset
                    .as_ref()
                    .ok_or_else(|| AdminError::unavailable("自动恢复不可用"))?
                    .arm(&command.group_ids, &command.model_fast_modes)
                    .await?,
            )
        } else {
            None
        };
        let id = ClientApiKeyId::new(format!("key_{}", Uuid::now_v7().simple()))
            .map_err(|_| AdminError::internal("创建 Client API Key ID 失败"))?;
        let plaintext = if let Some(key) = command.custom_key {
            key.expose_for_auth().to_owned()
        } else {
            generate_key()
        };
        let (config_revision, record) = self
            .store
            .create_client_key(
                NewClientKey {
                    fast_mode_reset,
                    model_fast_modes: command.model_fast_modes,
                    request_profile_overrides: command.request_profile_overrides,
                    id,
                    name: command.name,
                    label: command.label,
                    group_ids: command.group_ids,
                    limits: command.limits,
                    budget: command.budget,
                    plaintext: plaintext.clone(),
                },
                context,
            )
            .await
            .map_err(map_client_key_write_error)?;
        publish_committed(self.snapshot.as_ref(), config_revision).await?;
        Ok(CreatedClientKey {
            config_revision,
            secret: ClientKeySecret::new(record, plaintext),
        })
    }

    async fn update(
        &self,
        context: &MutationContext,
        command: UpdateClientKey,
    ) -> Result<ClientKeyMutation, AdminError> {
        crate::model::client_keys::validate_group_ids(&command.group_ids)
            .map_err(|_| AdminError::invalid("Client API Key 分组不合法"))?;
        let mut command = command;
        let current = self.get(&command.id).await?;
        command.expected_fast_mode_reset = Some(current.fast_mode_reset.clone());
        let now = chrono::Utc::now();
        let expired = current
            .fast_mode_reset
            .as_ref()
            .is_some_and(|plan| plan.expired(now));
        let prior_modes = if expired {
            current.model_fast_modes.without_enabled()
        } else {
            current.model_fast_modes.clone()
        };
        // 到期后省略模型字段也不能在取消计划时重新露出旧 enabled
        if expired && command.model_fast_modes.is_none() {
            command.model_fast_modes = Some(prior_modes.clone());
        }
        let modes = command.model_fast_modes.as_ref().unwrap_or(&prior_modes);
        let provider = gateway_core::identity::ProviderKind::new("openai")
            .map_err(|_| AdminError::internal("Provider 不合法"))?;
        let enabled = modes.enabled_models(&provider);
        let reenabled = enabled.iter().any(|model| {
            prior_modes.resolve(&provider, model, gateway_core::account::FastMode::Default)
                != gateway_core::account::FastMode::Enabled
        });
        let mut requested_groups = command.group_ids.clone();
        requested_groups.sort();
        let mut prior_groups: Vec<_> = current
            .groups
            .iter()
            .map(|group| group.id.clone())
            .collect();
        prior_groups.sort();
        let opted_in = command
            .reset_fast_mode_on_quota_reset
            .unwrap_or(current.fast_mode_reset.is_some());
        if !opted_in {
            command.fast_mode_reset_update = Some(None);
        } else if enabled.is_empty() {
            if let Some(mut plan) = current.fast_mode_reset.clone() {
                plan.completed_at.get_or_insert(now);
                command.fast_mode_reset_update = Some(Some(plan));
            } else {
                return Err(AdminError::invalid("请先单独启用模型 Fast，再设置自动恢复"));
            }
        } else if (command.reset_fast_mode_on_quota_reset == Some(true)
            && current.fast_mode_reset.is_none())
            || reenabled
            || requested_groups != prior_groups
        {
            command.fast_mode_reset_update = Some(Some(
                self.fast_reset
                    .as_ref()
                    .ok_or_else(|| AdminError::unavailable("自动恢复不可用"))?
                    .arm(&command.group_ids, modes)
                    .await?,
            ));
        }
        for (provider, profile) in &command.request_profile_override_updates {
            if let Some(profile) = profile {
                self.providers
                    .require(provider)
                    .and_then(|provider| provider.preview_client_profile(profile))
                    .map_err(|error| super::map_provider_error(error, "client profile"))?;
            }
        }
        let id = command.id.clone();
        let (config_revision, record) = self
            .store
            .update_client_key(command, context)
            .await
            .map_err(|error| {
                if error.kind() == AdminStoreErrorKind::Conflict {
                    AdminError::conflict("Fast 恢复计划已变化，请刷新后重试")
                } else {
                    map_client_key_write_error(error)
                }
            })?;
        publish_committed(self.snapshot.as_ref(), config_revision).await?;
        Ok(ClientKeyMutation {
            config_revision,
            record: Some(record),
            id,
        })
    }

    async fn set_enabled(
        &self,
        context: &MutationContext,
        command: SetClientKeyEnabled,
    ) -> Result<ClientKeyMutation, AdminError> {
        let id = command.id.clone();
        let (config_revision, record) =
            self.store
                .set_client_key_enabled(command, context)
                .await
                .map_err(|error| map_store_error(error, "client API key"))?;
        publish_committed(self.snapshot.as_ref(), config_revision).await?;
        Ok(ClientKeyMutation {
            config_revision,
            record: Some(record),
            id,
        })
    }

    async fn delete(
        &self,
        context: &MutationContext,
        command: DeleteClientKey,
    ) -> Result<ClientKeyMutation, AdminError> {
        let id = command.id.clone();
        let config_revision = self
            .store
            .delete_client_key(command, context)
            .await
            .map_err(|error| map_store_error(error, "client API key"))?;
        publish_committed(self.snapshot.as_ref(), config_revision).await?;
        Ok(ClientKeyMutation {
            config_revision,
            record: None,
            id,
        })
    }
}

fn map_client_key_write_error(error: AdminStoreError) -> AdminError {
    match error.kind() {
        AdminStoreErrorKind::DuplicateName => AdminError::conflict("名称已存在"),
        AdminStoreErrorKind::Conflict => AdminError::conflict("API Key 已存在，请使用其他密钥"),
        _ => map_store_error(error, "client API key"),
    }
}

fn validate_cursor(query: &ClientKeyListQuery) -> Result<(), AdminError> {
    let Some(cursor) = &query.cursor else {
        return Ok(());
    };
    if cursor.sort != query.sort {
        return Err(AdminError::invalid(
            "Client API Key 游标排序与查询条件不一致",
        ));
    }
    let matches = matches!(
        (cursor.sort.field, &cursor.value),
        (ClientKeySortField::Name, ClientKeyCursorValue::Name(value)) if !value.trim().is_empty()
    ) || matches!(
        (cursor.sort.field, &cursor.value),
        (
            ClientKeySortField::Enabled,
            ClientKeyCursorValue::Enabled(_)
        ) | (
            ClientKeySortField::CreatedAt,
            ClientKeyCursorValue::CreatedAt(_)
        ) | (
            ClientKeySortField::LastUsedAt,
            ClientKeyCursorValue::LastUsedAt(_)
        )
    );
    if matches {
        Ok(())
    } else {
        Err(AdminError::invalid("Client API Key 游标不合法"))
    }
}

// 原生与插件创建共用相同的密钥生成规则
pub(super) fn generate_key() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sk_{}", URL_SAFE_NO_PAD.encode(bytes))
}
