//! A run's own model and effort for its steps (ORC-322, `08 · Conversation · model and effort` `DPofN` and
//! `08f · Model and effort on a run · parts and rules` `Yzb3F`): a person changes the model or effort of the step at
//! work from its Conversation, for that step only or for it and every later step on the same connection. The run keeps
//! the choice; the pipeline and the agent profile never change from it. The runner starts the step's next turn with it,
//! so a change made while a turn runs waits for that turn to end, and can be canceled until then.

use serde::{Deserialize, Serialize};

use super::Answerer;
use crate::{
  resource::{ModelId, StepKey},
  time::Timestamp,
};

/// A model and the effort it runs with; no effort keeps the CLI's or the endpoint's default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChoice {
  pub model: ModelId,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
}

/// Which steps a change of model or effort holds for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelScope {
  /// From the step's next turn, for this step only: the next step starts on its own profile.
  Step,
  /// For this step and every later step whose agent runs on the same connection.
  Run,
}

/// What a step ran on before a change: the run's own choice for it then, if it had one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepBefore {
  pub step: StepKey,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub choice: Option<ModelChoice>,
}

/// What a run's Conversation records of a change of its model or effort.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ModelEventKind {
  /// The step's next turns run on `to` in place of `was`; `also` are the later steps the change holds for too, and
  /// `before` what each step it changed ran on before, which canceling it gives back.
  Changed {
    scope: ModelScope,
    to: ModelChoice,
    was: ModelChoice,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    also: Vec<StepKey>,
    before: Vec<StepBefore>,
  },
  /// A change that waited for a turn's end was canceled: the step stays on `stays`.
  Canceled { stays: ModelChoice },
}

impl ModelEventKind {
  /// Whether this is a change that only the effort of: the model stays.
  pub fn changes_effort_only(&self) -> bool {
    matches!(self, Self::Changed { to, was, .. } if to.model == was.model)
  }
}

/// A change of a run's model or effort, or its cancelation: when, by whom, for which step and what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelEvent {
  pub at: Timestamp,
  pub by: Answerer,
  pub step: StepKey,
  pub kind: ModelEventKind,
}
