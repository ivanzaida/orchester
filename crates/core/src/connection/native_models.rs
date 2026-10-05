//! The models a native connection offers (`docs/architecture/native-agent.md` §7): each with its
//! optional limits, the reasoning efforts it accepts and where it came from, plus the default model
//! and effort that profiles which choose none run with.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  native::{model::RemoteModel, session::DEFAULT_CONTEXT_WINDOW},
  resource::ModelId,
  time::Timestamp,
};

/// A reasoning effort a native model accepts, from the least to the most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
  Low,
  Medium,
  High,
  #[serde(rename = "xhigh")]
  XHigh,
  Max,
}

impl Effort {
  pub const ALL: [Self; 5] = [Self::Low, Self::Medium, Self::High, Self::XHigh, Self::Max];

  /// The value the model API takes, which is also how it is shown.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Low => "low",
      Self::Medium => "medium",
      Self::High => "high",
      Self::XHigh => "xhigh",
      Self::Max => "max",
    }
  }

  /// The effort `value` names, as [`Self::as_str`] writes it; `None` for one Orchester does not know.
  pub fn named(value: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|effort| effort.as_str() == value)
  }
}

/// Where a model and its limits come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "source")]
pub enum ModelSource {
  /// The endpoint listed it when it was fetched at `at`.
  Endpoint { at: Timestamp },
  /// The preset fills it in (DeepSeek's limits and efforts).
  Preset,
  /// A person added it by hand.
  Manual,
}

/// A model of a native connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeModel {
  id: ModelId,
  /// Tokens the model reads at most; unknown when `None`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  context_window: Option<NonZeroU32>,
  /// Tokens the model writes at most in one response; unknown when `None`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  max_output: Option<NonZeroU32>,
  /// Accepted efforts, least first; none: the model takes no effort setting.
  #[serde(default)]
  efforts: Vec<Effort>,
  #[serde(flatten)]
  source: ModelSource,
}

impl NativeModel {
  /// The efforts are kept once each, least first.
  pub fn new(
    id: ModelId,
    context_window: Option<NonZeroU32>,
    max_output: Option<NonZeroU32>,
    mut efforts: Vec<Effort>,
    source: ModelSource,
  ) -> Self {
    efforts.sort_unstable();
    efforts.dedup();
    Self {
      id,
      context_window,
      max_output,
      efforts,
      source,
    }
  }

  pub const fn id(&self) -> &ModelId {
    &self.id
  }

  pub const fn context_window(&self) -> Option<NonZeroU32> {
    self.context_window
  }

  pub const fn max_output(&self) -> Option<NonZeroU32> {
    self.max_output
  }

  pub fn efforts(&self) -> &[Effort] {
    &self.efforts
  }

  pub const fn source(&self) -> ModelSource {
    self.source
  }

  /// A model the endpoint listed when its models were fetched at `at`: efforts Orchester does not know
  /// and limits that do not fit are left out, so they read as unknown.
  pub fn listed(remote: &RemoteModel, at: Timestamp) -> Self {
    let fitting = |limit: u64| u32::try_from(limit).ok().and_then(NonZeroU32::new);
    Self::new(
      remote.id.clone(),
      remote.context_window.and_then(fitting),
      remote.max_output.and_then(NonZeroU32::new),
      remote.efforts.iter().filter_map(|effort| Effort::named(effort)).collect(),
      ModelSource::Endpoint { at },
    )
  }
}

/// `models` after a fetch found `fetched`: a model the endpoint listed replaces the one with its id, and
/// keeps its place; one listed for the first time follows; one the endpoint no longer lists stays as it
/// was, since a profile may run on it.
pub fn with_fetched(models: &[NativeModel], fetched: Vec<NativeModel>) -> Vec<NativeModel> {
  let mut merged: Vec<NativeModel> = models
    .iter()
    .map(|model| {
      fetched
        .iter()
        .find(|listed| listed.id == model.id)
        .unwrap_or(model)
        .clone()
    })
    .collect();
  merged.extend(
    fetched
      .into_iter()
      .filter(|listed| !models.iter().any(|model| model.id == listed.id)),
  );
  merged
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NativeModelsError {
  #[error("model {} is listed twice", .0.as_str())]
  Duplicate(ModelId),
  #[error("the default model {} is not listed", .0.as_str())]
  DefaultNotListed(ModelId),
  #[error("the default model does not take effort {}", .0.as_str())]
  EffortNotOffered(Effort),
  #[error("a default effort needs a default model")]
  EffortWithoutModel,
  #[error("model {} is used by a profile and must stay listed", .0.as_str())]
  UsedModelDropped(ModelId),
}

/// Checks that the models `after` still list every model in `used` (the models profiles and steps on
/// the connection run with) that the models `before` listed: removing such a model, or changing its id,
/// would leave its user without a model, so the user has to switch to another model first (decision of
/// 2026-10-01). A used model `before` did not list is not the list's to keep: a profile draft may name a
/// model the connection never listed, and that does not hold up any change of the list.
pub fn ensure_used_kept<'a>(
  before: &[NativeModel],
  after: &[NativeModel],
  used: impl IntoIterator<Item = &'a ModelId>,
) -> Result<(), NativeModelsError> {
  let listed = |models: &[NativeModel], id: &ModelId| models.iter().any(|model| &model.id == id);
  match used.into_iter().find(|id| listed(before, id) && !listed(after, id)) {
    Some(dropped) => Err(NativeModelsError::UsedModelDropped(dropped.clone())),
    None => Ok(()),
  }
}

/// The models of a native connection, with the default model and effort. Every id is listed once,
/// the default model is listed, and the default effort is one the default model takes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredModels", into = "StoredModels")]
pub struct NativeModels {
  models: Vec<NativeModel>,
  default_model: Option<ModelId>,
  default_effort: Option<Effort>,
}

impl NativeModels {
  pub fn new(
    models: Vec<NativeModel>,
    default_model: Option<ModelId>,
    default_effort: Option<Effort>,
  ) -> Result<Self, NativeModelsError> {
    for (index, model) in models.iter().enumerate() {
      if models[..index].iter().any(|earlier| earlier.id == model.id) {
        return Err(NativeModelsError::Duplicate(model.id.clone()));
      }
    }
    let chosen = match &default_model {
      Some(id) => Some(
        models
          .iter()
          .find(|model| &model.id == id)
          .ok_or_else(|| NativeModelsError::DefaultNotListed(id.clone()))?,
      ),
      None => None,
    };
    match (chosen, default_effort) {
      (None, Some(_)) => return Err(NativeModelsError::EffortWithoutModel),
      (Some(model), Some(effort)) if !model.efforts.contains(&effort) => {
        return Err(NativeModelsError::EffortNotOffered(effort));
      }
      _ => {}
    }
    Ok(Self {
      models,
      default_model,
      default_effort,
    })
  }

  pub fn models(&self) -> &[NativeModel] {
    &self.models
  }

  pub fn is_empty(&self) -> bool {
    self.models.is_empty()
  }

  pub fn model(&self, id: &ModelId) -> Option<&NativeModel> {
    self.models.iter().find(|model| &model.id == id)
  }

  pub const fn default_model(&self) -> Option<&ModelId> {
    self.default_model.as_ref()
  }

  pub const fn default_effort(&self) -> Option<Effort> {
    self.default_effort
  }

  /// The effort an agent on `model` runs with: `chosen`, the profile's, when it names an effort the
  /// model takes (any known one for a model the list does not have), else the default effort when the
  /// model takes it; `None` leaves the endpoint's own default.
  pub fn effort_for(&self, model: &ModelId, chosen: Option<&str>) -> Option<Effort> {
    let listed = self.model(model);
    let takes = |effort: &Effort| listed.is_none_or(|model| model.efforts.contains(effort));
    chosen
      .and_then(Effort::named)
      .filter(takes)
      .or_else(|| {
        self
          .default_effort
          .filter(|effort| listed.is_some_and(|model| model.efforts.contains(effort)))
      })
  }

  /// The context window of `model` in tokens: as listed, else [`DEFAULT_CONTEXT_WINDOW`] (§6.3).
  pub fn context_window(&self, model: &ModelId) -> u64 {
    self
      .model(model)
      .and_then(NativeModel::context_window)
      .map_or(DEFAULT_CONTEXT_WINDOW, |window| u64::from(window.get()))
  }
}

/// [`NativeModels`] as stored, checked when read back.
#[derive(Serialize, Deserialize)]
struct StoredModels {
  #[serde(default)]
  models: Vec<NativeModel>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  default_model: Option<ModelId>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  default_effort: Option<Effort>,
}

impl TryFrom<StoredModels> for NativeModels {
  type Error = NativeModelsError;

  fn try_from(stored: StoredModels) -> Result<Self, NativeModelsError> {
    Self::new(stored.models, stored.default_model, stored.default_effort)
  }
}

impl From<NativeModels> for StoredModels {
  fn from(models: NativeModels) -> Self {
    Self {
      models: models.models,
      default_model: models.default_model,
      default_effort: models.default_effort,
    }
  }
}
