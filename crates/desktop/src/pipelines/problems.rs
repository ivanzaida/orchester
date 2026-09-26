//! User-facing messages for what blocks publishing a pipeline, for field values that cannot be
//! used, and for failed saves and publishes. Steps are named by their names, not their keys.

use orchester_core::resource::{
  EditProblem, PipelineEdit, PipelineError, PromptError, PublishError, StepKey, StepKind, VersionNumber,
};
use orchester_store::StoreError;

use crate::messages::{self, Message};

/// A problem as the editor shows it: the message, the step selecting it jumps to, and the icon
/// kind of its button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProblemView {
  pub message: Message,
  pub step: Option<StepKey>,
  pub kind: ProblemKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemKind {
  NoSteps,
  Cycle,
  Rule,
  /// An agent step without a profile.
  Unassigned,
  /// A tool step without a tool.
  UnassignedTool,
}

impl lurq::app::component::DevtoolsInspectable for ProblemView {}

fn step_name(edit: &PipelineEdit, key: &StepKey) -> String {
  edit
    .step(key)
    .map_or_else(|| key.as_str().to_owned(), |step| step.name.as_str().to_owned())
}

pub fn problem_view(edit: &PipelineEdit, problem: &EditProblem, then: &str) -> ProblemView {
  match problem {
    EditProblem::NoSteps => ProblemView {
      message: Message::new("pipelines.problems.noSteps"),
      step: None,
      kind: ProblemKind::NoSteps,
    },
    EditProblem::Cycle(path) => {
      let names: Vec<String> = path
        .iter()
        .chain(path.first())
        .map(|key| step_name(edit, key))
        .collect();
      ProblemView {
        message: Message::new("pipelines.problems.cycle").with("path", names.join(&format!(" {then} "))),
        step: path.first().cloned(),
        kind: ProblemKind::Cycle,
      }
    }
    EditProblem::Rule(error) => ProblemView {
      message: rule_message(edit, error),
      step: rule_step(error).cloned(),
      kind: ProblemKind::Rule,
    },
    EditProblem::Unassigned(key) => {
      let tool = matches!(edit.step(key).map(|step| &step.kind), Some(StepKind::Tool { .. }));
      let (text, kind) = if tool {
        ("pipelines.problems.noTool", ProblemKind::UnassignedTool)
      } else {
        ("pipelines.problems.noProfile", ProblemKind::Unassigned)
      };
      ProblemView {
        message: Message::new(text).with("step", step_name(edit, key)),
        step: Some(key.clone()),
        kind,
      }
    }
  }
}

fn rule_step(error: &PipelineError) -> Option<&StepKey> {
  match error {
    PipelineError::DuplicateStep(key) | PipelineError::Cycle(key) => Some(key),
    PipelineError::UnknownDependency { step, .. }
    | PipelineError::UnknownRetryTarget { step, .. }
    | PipelineError::RetryTargetNotUpstream { step, .. } => Some(step),
  }
}

fn rule_message(edit: &PipelineEdit, error: &PipelineError) -> Message {
  let name = |key: &StepKey| step_name(edit, key);
  let about = |key: &StepKey, text: &'static str| Message::new(text).with("step", name(key));
  match error {
    PipelineError::DuplicateStep(key) => about(key, "pipelines.problems.duplicateStep"),
    PipelineError::UnknownDependency { step, .. } => about(step, "pipelines.problems.unknownDependency"),
    PipelineError::Cycle(key) => about(key, "pipelines.problems.cycleAt"),
    PipelineError::UnknownRetryTarget { step, .. } => about(step, "pipelines.problems.unknownRetryTarget"),
    PipelineError::RetryTargetNotUpstream { step, target } => Message::new("pipelines.problems.retryTargetNotUpstream")
      .with("step", name(step))
      .with("target", name(target)),
  }
}

pub fn publish_error(error: &StoreError) -> Message {
  match error {
    StoreError::Publish(PublishError::StaleDraft { based_on, latest }) => {
      let version = |number: &Option<VersionNumber>| number.map_or_else(String::new, |number| number.to_string());
      Message::new("pipelines.problems.staleDraft")
        .with("basedOn", version(based_on))
        .with("latest", version(latest))
    }
    StoreError::Publish(PublishError::UnassignedSteps(_) | PublishError::NoSteps) => {
      Message::new("pipelines.problems.notPublishable")
    }
    StoreError::Publish(PublishError::VersionLimit) => Message::new("pipelines.problems.versionLimit"),
    StoreError::NoDraft(_) => Message::new("pipelines.problems.noDraft"),
    other => messages::store_error(other),
  }
}

pub fn prompt_error(error: &PromptError) -> Message {
  match error {
    PromptError::UnclosedPlaceholder { .. } => Message::new("pipelines.problems.unclosedPlaceholder"),
    PromptError::UnknownVariable { name, .. } => {
      Message::new("pipelines.problems.unknownVariable").with("name", name.clone())
    }
  }
}
