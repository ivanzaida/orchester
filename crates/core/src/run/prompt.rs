//! The prompt an agent step starts with, assembled in the design's order (agent role → step
//! instructions → task context; `docs/design/orchester-workspace.md`, "Instructions"). Project,
//! pipeline and step instruction resources are not part of the prompt yet.
//!
//! Placeholders resolve against the run; a variable without a value in this run is an error
//! that names it, never an empty string.

use std::fmt::Write as _;

use thiserror::Error;

use super::state::Run;
use crate::{
  id::{ResourceId, StepExecutionId},
  project::Project,
  resource::{AgentProfile, PromptText, Step, Variable, VersionNumber},
  task::Task,
};

/// Which part of the prompt referred to a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptSource {
  RoleInstructions,
  StepInstructions,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptAssemblyError {
  #[error("the {part:?} use `{{{{{name}}}}}`, which has no value in this run", name = variable.name())]
  UnavailableVariable { variable: Variable, part: PromptSource },
}

/// The pipeline a run executes, as prompts name it.
#[derive(Clone, Copy, Debug)]
pub struct PipelineInfo<'a> {
  pub id: ResourceId,
  pub name: &'a str,
  pub version: VersionNumber,
}

/// Everything one step's prompt is made of.
#[derive(Clone, Copy, Debug)]
pub struct PromptInput<'a> {
  pub project: &'a Project,
  pub task: &'a Task,
  pub run: &'a Run,
  pub execution: StepExecutionId,
  pub pipeline: PipelineInfo<'a>,
  pub step: &'a Step,
  pub profile_id: ResourceId,
  pub profile: &'a AgentProfile,
}

impl PromptInput<'_> {
  /// The prompt text sent to the agent CLI.
  pub fn assemble(&self) -> Result<String, PromptAssemblyError> {
    let role = self.render(self.profile.instructions(), PromptSource::RoleInstructions)?;
    let step = self.render(&self.step.instructions, PromptSource::StepInstructions)?;
    let details = self.task.details();
    let mut prompt = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(prompt, "# Your role: {}", self.profile.role().as_str());
    push_block(&mut prompt, self.profile.responsibilities());
    push_block(&mut prompt, &role);
    let _ = writeln!(prompt, "\n# Current step: {}", self.step.name.as_str());
    push_block(&mut prompt, &step);
    let _ = writeln!(prompt, "\n# Task {}: {}", self.task.key(), details.title.as_str());
    push_block(&mut prompt, &details.brief);
    if !details.acceptance_criteria.is_empty() {
      prompt.push_str("\n## Acceptance criteria\n");
      prompt.push_str(&criteria(&details.acceptance_criteria));
    }
    Ok(prompt)
  }

  fn render(&self, text: &PromptText, source: PromptSource) -> Result<String, PromptAssemblyError> {
    text
      .render(|variable| self.value(variable))
      .map_err(|variable| PromptAssemblyError::UnavailableVariable { variable, part: source })
  }

  /// The value of `variable` in this run; `None` when the run has none.
  pub fn value(&self, variable: Variable) -> Option<String> {
    let details = self.task.details();
    let attempt = self
      .run
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(self.execution).is_some());
    Some(match variable {
      Variable::TaskId => self.task.key().to_string(),
      Variable::TaskTitle => details.title.as_str().to_owned(),
      Variable::TaskDescription => details.brief.clone(),
      Variable::TaskAcceptanceCriteria => criteria(&details.acceptance_criteria),
      Variable::ProjectId => self.project.id().to_string(),
      Variable::ProjectName => self.project.name().as_str().to_owned(),
      Variable::AgentId => self.profile_id.to_string(),
      Variable::AgentName | Variable::AgentRole => self.profile.role().as_str().to_owned(),
      Variable::AgentModel => self.profile.model().as_str().to_owned(),
      Variable::PipelineId => self.pipeline.id.to_string(),
      Variable::PipelineName => self.pipeline.name.to_owned(),
      Variable::PipelineVersion => format!("v{}", self.pipeline.version),
      Variable::RunId => self.run.id().to_string(),
      Variable::RunAttempt => attempt?.number().to_string(),
      Variable::RunBranch => self.run.branch().as_str().to_owned(),
      Variable::RunRevision => self.run.base().as_str().to_owned(),
      Variable::StepId => self.step.key.as_str().to_owned(),
      Variable::StepName => self.step.name.as_str().to_owned(),
      // Not known to a run yet: tasks have no attachments, the project records no repository
      // or default branch, creation times have no agreed text form, and budgets are changing
      // from money to tokens.
      Variable::TaskAttachments
      | Variable::TaskCreatedAt
      | Variable::ProjectRepository
      | Variable::ProjectDefaultBranch
      | Variable::RunBudget => return None,
    })
  }
}

fn push_block(prompt: &mut String, text: &str) {
  let text = text.trim();
  if !text.is_empty() {
    prompt.push('\n');
    prompt.push_str(text);
    prompt.push('\n');
  }
}

fn criteria(criteria: &[String]) -> String {
  criteria.iter().map(|criterion| format!("- {criterion}\n")).collect()
}
