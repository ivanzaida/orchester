//! The Anthropic Models API (`docs/architecture/native-agent.md` §3.1): `GET /v1/models`, page by page,
//! and `GET /v1/models/{id}`, read for the context window, the output cap, the effort levels and
//! whether the model thinks adaptively. Both are free.

use orchester_core::{native::model::RemoteModel, resource::ModelId};
use serde::Deserialize;
use serde_json::Value;

/// Effort levels in the order a picker shows them; other levels the endpoint names follow.
const EFFORT_ORDER: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// One model as the Models API describes it.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct ModelObject {
  id: String,
  max_input_tokens: Option<u64>,
  max_tokens: Option<u64>,
  #[serde(default)]
  capabilities: Value,
}

/// One page of `GET /v1/models`.
#[derive(Debug, Deserialize)]
pub(super) struct ModelPage {
  pub data: Vec<ModelObject>,
  #[serde(default)]
  pub has_more: bool,
  pub last_id: Option<String>,
}

/// What a request to a model must know about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModelTraits {
  /// The model takes `thinking: {type: "adaptive"}`; models without it run without thinking.
  pub adaptive_thinking: bool,
  /// The most output tokens one response may have.
  pub max_output: Option<u32>,
}

impl ModelObject {
  pub(super) fn traits(&self) -> ModelTraits {
    ModelTraits {
      adaptive_thinking: supported(&self.capabilities["thinking"]["types"]["adaptive"]),
      max_output: self.max_output(),
    }
  }

  /// The model as Orchester lists it; `None` for an id Orchester cannot use.
  pub(super) fn remote(&self) -> Option<RemoteModel> {
    Some(RemoteModel {
      id: ModelId::new(&self.id).ok()?,
      context_window: self.max_input_tokens,
      max_output: self.max_output(),
      efforts: efforts(&self.capabilities["effort"]),
    })
  }

  fn max_output(&self) -> Option<u32> {
    self.max_tokens.and_then(|tokens| u32::try_from(tokens).ok())
  }
}

fn supported(capability: &Value) -> bool {
  capability["supported"].as_bool().unwrap_or(false)
}

/// The supported levels of an `effort` capability, in [`EFFORT_ORDER`].
fn efforts(effort: &Value) -> Vec<String> {
  if !supported(effort) {
    return Vec::new();
  }
  let Some(levels) = effort.as_object() else {
    return Vec::new();
  };
  let mut found: Vec<&String> = levels
    .iter()
    .filter(|(name, level)| name.as_str() != "supported" && supported(level))
    .map(|(name, _)| name)
    .collect();
  found.sort_by_key(|name| {
    EFFORT_ORDER
      .iter()
      .position(|known| known == name)
      .unwrap_or(EFFORT_ORDER.len())
  });
  found.into_iter().cloned().collect()
}
