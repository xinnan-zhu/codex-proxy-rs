//! 独立 Key Fast 恢复任务，不在推理请求刷新上游额度

use crate::{
    model::{MutationActor, MutationContext, client_keys::ClientKeyRecord},
    ports::store::ClientKeyStore,
    use_case::client_keys::fast_reset::{AccountObservations, FastResetSource, in_scope},
};
use chrono::Utc;
use futures::future::BoxFuture;
use gateway_core::{
    runtime::SnapshotControl,
    task::{ScheduledTask, WorkerCycleContext, WorkerTaskError},
};
use std::{sync::Arc, time::Duration};

pub(crate) struct ClientKeyFastResetTask {
    pub source: FastResetSource,
    pub keys: Arc<dyn ClientKeyStore>,
    pub snapshot: Arc<dyn SnapshotControl>,
}

impl ScheduledTask for ClientKeyFastResetTask {
    fn run_cycle(&self, context: WorkerCycleContext) -> BoxFuture<'_, Result<(), WorkerTaskError>> {
        Box::pin(async move {
            let work = async {
                let records = self
                    .keys
                    .pending_fast_mode_resets(100)
                    .await
                    .map_err(|_| WorkerTaskError::safe("Key Fast 计划读取失败"))?;
                if records.is_empty() {
                    return Ok(());
                }
                let mut failed = false;
                let now = Utc::now();
                let (expired, pending): (Vec<_>, Vec<_>) =
                    records.into_iter().partition(|record| {
                        record
                            .fast_mode_reset
                            .as_ref()
                            .is_some_and(|plan| plan.expired(now))
                    });
                // 正常到期先提交，额度服务不可用或超时不会拖延定时恢复
                for record in expired {
                    if let Err(error) = self.advance(record, &[]).await {
                        failed = true;
                        tracing::warn!(error = %error, "Key Fast 到期恢复失败");
                    }
                }
                if pending.is_empty() {
                    return if failed {
                        Err(WorkerTaskError::safe("Key Fast 恢复存在失败"))
                    } else {
                        Ok(())
                    };
                }
                let observations =
                    match tokio::time::timeout(Duration::from_secs(10), self.source.observations())
                        .await
                    {
                        Ok(Ok(observations)) => observations,
                        _ => {
                            failed = true;
                            Vec::new()
                        }
                    };
                for record in pending {
                    if let Err(error) = self.advance(record, &observations).await {
                        failed = true;
                        tracing::warn!(error = %error, "Key Fast 提前恢复检查失败");
                    }
                }
                if failed {
                    Err(WorkerTaskError::safe("Key Fast 额度观测或恢复存在失败"))
                } else {
                    Ok(())
                }
            };
            tokio::select! {
                () = context.cancellation().cancelled() => Ok(()),
                result = work => result,
            }
        })
    }
}

impl ClientKeyFastResetTask {
    async fn advance(
        &self,
        record: ClientKeyRecord,
        observations: &[AccountObservations],
    ) -> Result<(), WorkerTaskError> {
        let Some(expected) = record.fast_mode_reset else {
            return Ok(());
        };
        let groups: Vec<_> = record
            .groups
            .iter()
            .filter(|group| group.enabled)
            .map(|group| group.id.clone())
            .collect();
        // 存在绑定但全部禁用时不退回全账号范围
        let current: Vec<_> = observations
            .iter()
            .filter(|item| {
                (record.groups.is_empty() || !groups.is_empty())
                    && in_scope(item, &groups, &record.model_fast_modes)
            })
            .flat_map(|item| item.observations.iter().cloned())
            .collect();
        let now = Utc::now();
        let mut next = expected.clone();
        let restore = next.observe(&current, now);
        if restore {
            next.completed_at = Some(now);
        }
        let context = MutationContext {
            actor: MutationActor::System,
            request_id: format!("key-fast-reset:{}", expected.generation),
        };
        if let Some(revision) = self
            .keys
            .advance_fast_mode_reset(&record.id, &expected, &next, restore, &context)
            .await
            .map_err(|_| WorkerTaskError::safe("Key Fast 恢复写入失败"))?
        {
            crate::use_case::publish_committed(self.snapshot.as_ref(), revision)
                .await
                .map_err(|_| WorkerTaskError::safe("Key Fast 快照发布失败"))?;
        }
        Ok(())
    }
}
