//! 系统更新出站代理选择；只保存已登记代理的 ID，不进入运行时快照。

use super::{AdminAuditEvent, PgControlPlaneRepository, append_admin_audit_event_in_transaction};
use crate::{Revision, StoreError, StoreResult, postgres_unavailable};

impl PgControlPlaneRepository {
    pub async fn load_system_update_proxy_id(&self) -> StoreResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            "select system_update_proxy_id from runtime_settings where id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| postgres_unavailable("load system update proxy"))?
        .ok_or_else(|| StoreError::NotFound {
            entity: "runtime settings",
            id: "1".to_owned(),
        })
    }

    pub async fn replace_system_update_proxy_id(
        &self,
        proxy_id: Option<String>,
        audit: AdminAuditEvent,
    ) -> StoreResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| postgres_unavailable("begin system update proxy mutation"))?;
        let updated = sqlx::query(
            "update runtime_settings
             set system_update_proxy_id = $1, updated_at = now()
             where id = 1",
        )
        .bind(proxy_id.as_deref())
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .is_some_and(sqlx::error::DatabaseError::is_foreign_key_violation)
            {
                StoreError::NotFound {
                    entity: "outbound proxy",
                    id: proxy_id.clone().unwrap_or_default(),
                }
            } else {
                postgres_unavailable("update system update proxy")
            }
        })?;
        if updated.rows_affected() == 0 {
            return Err(StoreError::NotFound {
                entity: "runtime settings",
                id: "1".to_owned(),
            });
        }
        append_admin_audit_event_in_transaction(&mut transaction, audit, None::<Revision>).await?;
        transaction
            .commit()
            .await
            .map_err(|_| postgres_unavailable("commit system update proxy mutation"))
    }
}
