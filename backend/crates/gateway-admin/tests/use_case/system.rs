use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use gateway_admin::{
    model::{
        MutationActor, MutationContext, Revision,
        proxies::ProxyRecord,
        settings::{AdminApiKey, AdminApiKeyMutation, ReplaceRuntimeSettings, RuntimeSettings},
        system::{
            SystemOperationAccepted, SystemOperationState, SystemOperationStatus,
            SystemUpdateDetail, SystemUpdateStatus, SystemVersion,
        },
    },
    ports::{
        store::{AdminStoreError, AdminStoreErrorKind, AdminStoreResult, SettingsStore},
        system::{SystemOperationError, SystemOperations, SystemUpdateEventStream},
    },
};
use gateway_core::account::OutboundProxy;

use super::proxies::TestProxies;

#[derive(Default)]
struct RecordingSystemOperations {
    target: Mutex<Option<Option<String>>>,
    proxy_endpoint: Mutex<Option<Option<String>>>,
    channel: Mutex<Option<gateway_admin::model::system::SystemUpdateChannel>>,
}

#[async_trait]
impl SystemOperations for RecordingSystemOperations {
    async fn version(&self) -> Result<SystemVersion, SystemOperationError> {
        Ok(SystemVersion {
            version: "1.0.0".to_owned(),
            git_sha: "unknown".to_owned(),
            build_time: "unknown".to_owned(),
            deployment_mode: "source".to_owned(),
            update_channel: "stable".to_owned(),

            latest_version: "1.0.0".to_owned(),
            has_update: false,
            update_cached: false,
            update_warning: None,
        })
    }

    async fn update_detail(
        &self,
        _: bool,
        _: Option<gateway_admin::model::system::SystemUpdateChannel>,
    ) -> Result<SystemUpdateDetail, SystemOperationError> {
        Ok(SystemUpdateDetail {
            policy: gateway_admin::model::system::SystemUpdatePolicy {
                channel: gateway_admin::model::system::SystemUpdateChannel::Stable,
                available_channels: vec![gateway_admin::model::system::SystemUpdateChannel::Stable],
            },
            current_version: "1.0.0".to_owned(),
            latest_version: "1.0.0".to_owned(),
            has_update: false,
            deployment_mode: "source".to_owned(),
            build_type: "source".to_owned(),
            release_url: None,
            notes: None,
            cached: false,
            update_supported: false,
            unsupported_reason: Some("source build".to_owned()),
            warning: None,
        })
    }

    fn update_events(&self) -> SystemUpdateEventStream {
        Box::pin(futures::stream::empty())
    }

    async fn perform_update(
        &self,
        target_version: Option<String>,
        channel: Option<gateway_admin::model::system::SystemUpdateChannel>,
        _: std::sync::Arc<dyn gateway_admin::ports::system::SystemUpdatePreflight>,
    ) -> Result<SystemOperationAccepted, SystemOperationError> {
        *self.target.lock().expect("target") = Some(target_version.clone());
        *self.channel.lock().expect("channel") = channel;
        Ok(SystemOperationAccepted::Update {
            operation_id: "operation-update".to_owned(),
            deployment_mode: "source".to_owned(),
            message: "accepted".to_owned(),
            target_version: target_version.unwrap_or_else(|| "latest".to_owned()),
        })
    }

    async fn update_status(&self) -> Result<SystemUpdateStatus, SystemOperationError> {
        Ok(SystemUpdateStatus {
            need_restart: false,
            previous_version: None,
            current_version: Some("1.0.0".to_owned()),
            operation: SystemOperationState {
                operation_id: None,
                kind: None,
                status: SystemOperationStatus::Idle,
                target_version: None,
                message: None,
                error: None,
                started_at: None,
                finished_at: None,
            },
        })
    }

    async fn rollback(
        &self,
        _: std::sync::Arc<dyn gateway_admin::ports::system::SystemUpdatePreflight>,
    ) -> Result<SystemOperationAccepted, SystemOperationError> {
        Ok(SystemOperationAccepted::Rollback {
            operation_id: "operation-rollback".to_owned(),
            message: "accepted".to_owned(),
            need_restart: true,
        })
    }

    async fn restart(&self) -> Result<SystemOperationAccepted, SystemOperationError> {
        Ok(SystemOperationAccepted::Restart {
            operation_id: "operation-restart".to_owned(),
            message: "accepted".to_owned(),
        })
    }

    fn set_update_proxy(&self, proxy: Option<OutboundProxy>) {
        *self.proxy_endpoint.lock().expect("proxy") =
            Some(proxy.as_ref().map(OutboundProxy::endpoint));
    }
}

#[derive(Default)]
struct UpdateProxySettingsStore {
    proxy_id: Mutex<Option<String>>,
}

fn unused() -> AdminStoreError {
    AdminStoreError::new(
        AdminStoreErrorKind::Unavailable,
        "settings",
        "unused in this test",
    )
}

#[async_trait]
impl SettingsStore for UpdateProxySettingsStore {
    async fn load_pricing(&self) -> AdminStoreResult<gateway_admin::model::pricing::StoredPricing> {
        Err(unused())
    }

    async fn sync_pricing(
        &self,
        _: gateway_admin::model::pricing::PricingSyncChanges,
        _: &MutationContext,
    ) -> AdminStoreResult<Revision> {
        Err(unused())
    }

    async fn update_pricing(
        &self,
        _: gateway_admin::model::pricing::UpdatePricing,
        _: &MutationContext,
    ) -> AdminStoreResult<Revision> {
        Err(unused())
    }

    async fn load_runtime_settings(&self) -> AdminStoreResult<RuntimeSettings> {
        Err(unused())
    }

    async fn admin_api_key_exists(&self) -> AdminStoreResult<bool> {
        Err(unused())
    }

    async fn replace_runtime_settings(
        &self,
        _: ReplaceRuntimeSettings,
        _: &MutationContext,
    ) -> AdminStoreResult<RuntimeSettings> {
        Err(unused())
    }

    async fn replace_admin_api_key(
        &self,
        _: AdminApiKey,
        _: &MutationContext,
    ) -> AdminStoreResult<AdminApiKeyMutation> {
        Err(unused())
    }

    async fn delete_admin_api_key(
        &self,
        _: &MutationContext,
    ) -> AdminStoreResult<AdminApiKeyMutation> {
        Err(unused())
    }

    async fn load_system_update_proxy_id(&self) -> AdminStoreResult<Option<String>> {
        Ok(self.proxy_id.lock().expect("proxy id").clone())
    }

    async fn replace_system_update_proxy_id(
        &self,
        proxy_id: Option<String>,
        _: &MutationContext,
    ) -> AdminStoreResult<()> {
        *self.proxy_id.lock().expect("proxy id") = proxy_id;
        Ok(())
    }
}

fn update_proxy_record(id: &str, url: &str) -> ProxyRecord {
    let now = chrono::Utc::now();
    ProxyRecord {
        auto_location: false,
        detected_location: None,
        location: None,
        id: id.to_owned(),
        name: "更新出口".to_owned(),
        proxy: OutboundProxy::parse(url).expect("proxy"),
        revision: Revision::new(1).expect("revision"),
        account_count: 0,
        last_test_at: None,
        last_test: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn system_update_should_normalize_blank_target_to_latest() {
    let operations = std::sync::Arc::new(RecordingSystemOperations::default());
    let services = super::AdminHarness::new()
        .system(operations.clone())
        .build()
        .await;
    services
        .system()
        .perform_update(Some("   ".to_owned()), None)
        .await
        .expect("perform update");

    assert_eq!(*operations.target.lock().expect("target"), Some(None));
}

#[tokio::test]
async fn system_update_proxy_should_persist_and_route_release_checks() {
    let operations = Arc::new(RecordingSystemOperations::default());
    let settings = Arc::new(UpdateProxySettingsStore::default());
    let services = super::AdminHarness::new()
        .system(operations.clone())
        .settings(settings.clone())
        .proxies(Arc::new(TestProxies {
            record: Some(update_proxy_record(
                "proxy_update",
                "socks5h://user:secret@127.0.0.1:1080",
            )),
            ..Default::default()
        }))
        .build()
        .await;
    let context = MutationContext {
        actor: MutationActor::System,
        request_id: "update-proxy-test".to_owned(),
    };
    let expected = OutboundProxy::parse("socks5h://127.0.0.1:1080")
        .expect("proxy")
        .endpoint();

    let saved = services
        .system()
        .set_update_proxy(&context, Some(" proxy_update ".to_owned()))
        .await
        .expect("save proxy");
    assert_eq!(saved.as_deref(), Some("proxy_update"));
    assert_eq!(
        services
            .system()
            .update_proxy()
            .await
            .expect("load proxy")
            .as_deref(),
        Some("proxy_update")
    );

    *operations.proxy_endpoint.lock().expect("proxy") = None;
    services
        .system()
        .update_detail(true, None)
        .await
        .expect("update detail");
    assert_eq!(
        *operations.proxy_endpoint.lock().expect("proxy"),
        Some(Some(expected.clone()))
    );

    services
        .system()
        .set_update_proxy(&context, Some("proxy_missing".to_owned()))
        .await
        .expect_err("unknown proxy must be rejected");
    assert_eq!(
        settings.proxy_id.lock().expect("proxy id").as_deref(),
        Some("proxy_update")
    );

    services
        .system()
        .set_update_proxy(&context, None)
        .await
        .expect("switch to direct");
    services
        .system()
        .perform_update(Some("1.0.1".to_owned()), None)
        .await
        .expect("perform update");
    assert_eq!(
        *operations.proxy_endpoint.lock().expect("proxy"),
        Some(None)
    );
}

#[tokio::test]
async fn system_update_should_forward_the_confirmed_channel() {
    let operations = std::sync::Arc::new(RecordingSystemOperations::default());
    let services = super::AdminHarness::new()
        .system(operations.clone())
        .build()
        .await;
    let channel = gateway_admin::model::system::SystemUpdateChannel::Beta;
    services
        .system()
        .perform_update(Some(" 1.2.0-beta.1 ".into()), Some(channel))
        .await
        .expect("accepted");
    assert_eq!(
        *operations.target.lock().expect("target"),
        Some(Some("1.2.0-beta.1".into()))
    );
    assert_eq!(*operations.channel.lock().expect("channel"), Some(channel));
}
