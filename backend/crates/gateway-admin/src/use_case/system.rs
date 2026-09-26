//! 系统管理用例。

use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    model::{
        AdminError, AdminErrorKind, MutationContext,
        system::{
            SystemOperationAccepted, SystemUpdateChannel, SystemUpdateDetail, SystemUpdateStatus,
            SystemVersion,
        },
    },
    ports::{
        proxy::ProxyStore,
        store::{AdminStoreErrorKind, SettingsStore},
        system::{
            SystemOperationError, SystemOperationErrorKind, SystemOperations,
            SystemUpdateEventStream, SystemUpdatePreflight,
        },
    },
};

use super::map_store_error;

/// API 消费的系统管理服务。
#[async_trait]
pub trait SystemService: Send + Sync {
    async fn version(&self) -> Result<SystemVersion, AdminError>;
    async fn update_detail(
        &self,
        refresh: bool,
        channel: Option<SystemUpdateChannel>,
    ) -> Result<SystemUpdateDetail, AdminError>;
    fn update_events(&self) -> SystemUpdateEventStream;
    async fn perform_update(
        &self,
        target_version: Option<String>,
        channel: Option<SystemUpdateChannel>,
    ) -> Result<SystemOperationAccepted, AdminError>;
    async fn update_status(&self) -> Result<SystemUpdateStatus, AdminError>;
    async fn rollback(&self) -> Result<SystemOperationAccepted, AdminError>;
    async fn restart(&self) -> Result<SystemOperationAccepted, AdminError>;
    async fn update_proxy(&self) -> Result<Option<String>, AdminError>;
    async fn set_update_proxy(
        &self,
        context: &MutationContext,
        proxy_id: Option<String>,
    ) -> Result<Option<String>, AdminError>;
}

/// 保持 Host 能力窄边界的默认系统用例。
pub(crate) struct DefaultSystemService {
    operations: Arc<dyn SystemOperations>,
    preflight: Arc<dyn SystemUpdatePreflight>,
    settings: Arc<dyn SettingsStore>,
    proxies: Arc<dyn ProxyStore>,
}

impl DefaultSystemService {
    #[must_use]
    pub(crate) fn new(
        operations: Arc<dyn SystemOperations>,
        preflight: Arc<dyn SystemUpdatePreflight>,
        settings: Arc<dyn SettingsStore>,
        proxies: Arc<dyn ProxyStore>,
    ) -> Self {
        Self {
            operations,
            preflight,
            settings,
            proxies,
        }
    }

    /// 每次访问 GitHub 前按库内选择重新解析，代理地址被编辑或删除后立即生效。
    async fn sync_update_proxy(&self) -> Result<(), AdminError> {
        let proxy_id = self
            .settings
            .load_system_update_proxy_id()
            .await
            .map_err(|error| map_store_error(error, "system update proxy"))?;
        let proxy = match proxy_id {
            None => None,
            Some(id) => match self.proxies.get(&id).await {
                Ok(record) => Some(record.proxy),
                Err(error) if error.kind() == AdminStoreErrorKind::NotFound => None,
                Err(error) => return Err(map_store_error(error, "outbound proxy")),
            },
        };
        self.operations.set_update_proxy(proxy);
        Ok(())
    }
}

#[async_trait]
impl SystemService for DefaultSystemService {
    async fn version(&self) -> Result<SystemVersion, AdminError> {
        // 版本信息在重启期间被高频轮询，代理解析失败时沿用上一次的选择。
        if let Err(error) = self.sync_update_proxy().await {
            tracing::warn!(error = %error, "system update proxy unavailable; keeping previous");
        }
        self.operations.version().await.map_err(map_system_error)
    }

    async fn update_detail(
        &self,
        refresh: bool,
        channel: Option<SystemUpdateChannel>,
    ) -> Result<SystemUpdateDetail, AdminError> {
        self.sync_update_proxy().await?;
        self.operations
            .update_detail(refresh, channel)
            .await
            .map_err(map_system_error)
    }

    fn update_events(&self) -> SystemUpdateEventStream {
        self.operations.update_events()
    }

    async fn perform_update(
        &self,
        target_version: Option<String>,
        channel: Option<SystemUpdateChannel>,
    ) -> Result<SystemOperationAccepted, AdminError> {
        let target_version = target_version
            .map(|version| version.trim().to_owned())
            .filter(|version| !version.is_empty());
        self.sync_update_proxy().await?;
        self.operations
            .perform_update(target_version, channel, Arc::clone(&self.preflight))
            .await
            .map_err(map_system_error)
    }

    async fn update_status(&self) -> Result<SystemUpdateStatus, AdminError> {
        self.operations
            .update_status()
            .await
            .map_err(map_system_error)
    }

    async fn rollback(&self) -> Result<SystemOperationAccepted, AdminError> {
        self.operations
            .rollback(Arc::clone(&self.preflight))
            .await
            .map_err(map_system_error)
    }

    async fn restart(&self) -> Result<SystemOperationAccepted, AdminError> {
        self.operations.restart().await.map_err(map_system_error)
    }

    async fn update_proxy(&self) -> Result<Option<String>, AdminError> {
        self.settings
            .load_system_update_proxy_id()
            .await
            .map_err(|error| map_store_error(error, "system update proxy"))
    }

    async fn set_update_proxy(
        &self,
        context: &MutationContext,
        proxy_id: Option<String>,
    ) -> Result<Option<String>, AdminError> {
        let proxy_id = proxy_id
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty());
        if let Some(id) = &proxy_id {
            self.proxies
                .get(id)
                .await
                .map_err(|error| map_store_error(error, "outbound proxy"))?;
        }
        self.settings
            .replace_system_update_proxy_id(proxy_id.clone(), context)
            .await
            .map_err(|error| map_store_error(error, "system update proxy"))?;
        self.sync_update_proxy().await?;
        Ok(proxy_id)
    }
}

fn map_system_error(error: SystemOperationError) -> AdminError {
    let kind = match error.kind() {
        SystemOperationErrorKind::Invalid => AdminErrorKind::Invalid,
        SystemOperationErrorKind::Conflict => AdminErrorKind::Conflict,
        SystemOperationErrorKind::Upstream => AdminErrorKind::BadGateway,
        SystemOperationErrorKind::Internal => AdminErrorKind::Internal,
    };
    let message = match kind {
        AdminErrorKind::Invalid => "系统操作请求不合法",
        AdminErrorKind::Conflict => "系统当前状态不允许执行该操作",
        AdminErrorKind::BadGateway => "系统更新服务请求失败",
        AdminErrorKind::Internal => "系统操作失败",
        _ => "系统操作失败",
    };
    AdminError::new(kind, message)
}
