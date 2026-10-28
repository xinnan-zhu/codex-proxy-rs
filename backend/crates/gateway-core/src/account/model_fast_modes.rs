//! Client Key 按 Provider 与实际上游模型保存的 Fast 覆盖

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::{account::FastMode, identity::ProviderKind, validation::validate_text};

/// 精确匹配上游模型，不把公开别名或其他 Provider 的同名模型视作同一配置
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ModelFastModes(BTreeMap<ProviderKind, BTreeMap<String, FastMode>>);

#[derive(Debug, Clone, Copy, Error)]
#[error(
    "modelFastModes accepts at most 256 exact upstream model rules (model IDs at most 256 bytes)"
)]
pub struct InvalidModelFastModes;

impl ModelFastModes {
    pub fn new(
        rules: BTreeMap<ProviderKind, BTreeMap<String, FastMode>>,
    ) -> Result<Self, InvalidModelFastModes> {
        if rules.len() > 256
            || rules.values().map(BTreeMap::len).sum::<usize>() > 256
            || rules.values().flat_map(BTreeMap::keys).any(|model| {
                model.trim() != model
                    || model.contains('*')
                    || validate_text(model, 256, true, None).is_err()
            })
        {
            return Err(InvalidModelFastModes);
        }
        // default 只表达继承，不产生第二份默认策略
        let rules = rules
            .into_iter()
            .filter_map(|(provider, models)| {
                let models: BTreeMap<_, _> = models
                    .into_iter()
                    .filter(|(_, mode)| *mode != FastMode::Default)
                    .collect();
                (!models.is_empty()).then_some((provider, models))
            })
            .collect();
        Ok(Self(rules))
    }

    /// 自动恢复仅移除单独启用，保留显式禁用
    #[must_use]
    pub fn without_enabled(&self) -> Self {
        Self(
            self.0
                .iter()
                .filter_map(|(provider, models)| {
                    let kept: BTreeMap<_, _> = models
                        .iter()
                        .filter(|(_, mode)| **mode != FastMode::Enabled)
                        .map(|(model, mode)| (model.clone(), *mode))
                        .collect();
                    (!kept.is_empty()).then_some((provider.clone(), kept))
                })
                .collect(),
        )
    }

    #[must_use]
    pub fn enabled_models(&self, provider: &ProviderKind) -> Vec<&str> {
        self.0
            .get(provider)
            .into_iter()
            .flat_map(|models| models.iter())
            .filter_map(|(model, mode)| (*mode == FastMode::Enabled).then_some(model.as_str()))
            .collect()
    }

    #[must_use]
    pub fn resolve(&self, provider: &ProviderKind, model: &str, inherited: FastMode) -> FastMode {
        self.0
            .get(provider)
            .and_then(|models| models.get(model))
            .copied()
            .unwrap_or(inherited)
    }
}

impl<'de> Deserialize<'de> for ModelFastModes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(BTreeMap::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
