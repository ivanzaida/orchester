//! The prompt an agent invocation starts with, assembled in the design's order
//! (`docs/design/orchester-workspace.md`, "Instructions"): agent role (the profile, then agent role
//! instructions) → project rules → pipeline instructions → step instructions (the step's own, then
//! step instructions) → task context, then the outputs of the steps it runs after, each naming its
//! source, and how to report the result ([`super::StepResult`]).
//!
//! Instruction resources are injected by event: a new session ([`InjectionEvent::Start`]) gets the
//! whole prompt with the instructions for starting; a resumed session ([`InjectionEvent::Resume`])
//! already holds that prompt and gets [`CONTINUE_PROMPT`] with the instructions for resuming. A
//! gated step a reviewer sent back ([`PlanRevision`]) is told what should change instead: a resumed
//! session gets the reviewer's note in place of the continue prompt, a new one its earlier plan and
//! the note in the task part. People's messages an invocation delivers ([`DeliveredMessages`]) follow
//! the same rule: a resumed session is told them first, introduced by why it reads them now (the
//! answer to its question, a follow-up after the run finished, messages that came in while it
//! worked), and a new session reads them in the task part.
//!
//! A new session is also told the skills it gets ([`SkillListing`]): each one's name, description and
//! where its `SKILL.md` is, before the task part, so the agent reads a skill when its work needs it.
//! A resumed session was told them when it started, and the files stay where they were.
//!
//! New and resumed sessions alike are told about the code index ([`CodeIndexPrompt`]): where the index of the run's
//! base revision is and the internal skill that explains searching it, or that ripgrep is missing and `git grep -n`
//! searches instead. A resumed session is told again because the index may be new to it: the setting or ripgrep can
//! have changed since it started.
//!
//! Placeholders resolve against the run; a variable without a value is an error that names it,
//! never an empty string. A preview keeps such placeholders and lists them instead.

mod code_index;
mod composition;
mod source;

use std::num::NonZeroU32;

use thiserror::Error;

pub use self::{
  composition::{AssembledPrompt, Composition, ExcludedInstruction, PromptSection},
  source::{InstructionVersion, InvocationPrompt, PromptPart, PromptSource},
};
use super::{
  plan::{PlanVersion, ReviewNote},
  state::Run,
};
use crate::{
  conversation::Message,
  id::ResourceId,
  project::Project,
  resource::{AgentProfile, InjectionEvent, Instruction, Step, Variable, VersionNumber},
  task::Task,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptAssemblyError {
  #[error("{section:?} uses `{{{{{name}}}}}`, which has no value in this run", name = variable.name())]
  UnavailableVariable { variable: Variable, section: PromptSource },
}

/// The pipeline a run executes, as prompts name it.
#[derive(Clone, Copy, Debug)]
pub struct PipelineInfo<'a> {
  pub id: ResourceId,
  pub name: &'a str,
  pub version: VersionNumber,
}

/// The published agent profile version that runs the step.
#[derive(Clone, Copy, Debug)]
pub struct ProfileInfo<'a> {
  pub id: ResourceId,
  pub version: VersionNumber,
  pub body: &'a AgentProfile,
}

/// An instruction resource offered to the prompt; whether it is injected depends on its scope,
/// targets and events.
#[derive(Clone, Copy, Debug)]
pub struct InstructionSource<'a> {
  pub resource: ResourceId,
  pub name: &'a str,
  pub version: InstructionVersion,
  pub body: &'a Instruction,
}

/// A skill the agent gets: its name, what it is for, and where its `SKILL.md` was placed.
#[derive(Clone, Copy, Debug)]
pub struct SkillListing<'a> {
  pub name: &'a str,
  pub description: &'a str,
  pub path: &'a str,
}

/// What a prompt tells an agent about the code index (`crate::code_index`).
#[derive(Clone, Copy, Debug)]
pub enum CodeIndexPrompt<'a> {
  /// The index of the run's base `revision` is in `folder`; the skill whose `SKILL.md` is `skill`
  /// explains how to search it.
  Given {
    revision: &'a str,
    folder: &'a str,
    skill: &'a str,
  },
  /// ripgrep is not available to the agent: it searches tracked files with `git grep -n`.
  WithoutRipgrep,
}

/// The run an agent works in and the attempt its step execution belongs to.
#[derive(Clone, Copy, Debug)]
pub struct RunContext<'a> {
  pub run: &'a Run,
  /// `None` before the step has an execution in the run.
  pub attempt: Option<NonZeroU32>,
}

/// The output of a step the prompt's step runs after, and the execution it came from.
#[derive(Clone, Copy, Debug)]
pub struct DependencyOutput<'a> {
  pub step: &'a Step,
  pub attempt: NonZeroU32,
  /// Empty when the step's agent wrote no final message.
  pub text: &'a str,
}

/// A gated step's plan a reviewer sent back, and what should change: the next invocation of the
/// step revises it.
#[derive(Clone, Copy, Debug)]
pub struct PlanRevision<'a> {
  pub version: PlanVersion,
  /// The plan as the agent left it.
  pub plan: &'a str,
  pub note: &'a ReviewNote,
}

/// Why an invocation delivers people's messages, which decides how its prompt introduces them.
#[derive(Clone, Copy, Debug)]
pub enum MessageOccasion<'a> {
  /// They came in before the step started, or while it was stopped (a pause, a limit, a restart).
  Written,
  /// The agent finished its turn while they came in; its session resumes to read them.
  AfterTurn,
  /// They answer `question`, which the agent asked.
  Answer { question: &'a str },
  /// The run had finished and a person asks the agent of the step about its work (Ask agent).
  FollowUp,
  /// A person's new instructions recover a failed attempt; retain its result as context.
  Recovery { previous_result: &'a str },
}

/// People's messages an invocation delivers, oldest first, and why it delivers them now.
#[derive(Clone, Copy, Debug)]
pub struct DeliveredMessages<'a> {
  pub occasion: MessageOccasion<'a>,
  pub messages: &'a [Message],
}

/// What a continued agent session is told when it resumes a step it did not finish: after a
/// pause, a stop at a limit or a restart. Its earlier conversation holds the step's prompt.
pub const CONTINUE_PROMPT: &str = "You were stopped before you finished this step. Everything you changed so far \
                                   is committed in the worktree. Continue the step from where you stopped and \
                                   finish it.";

/// Everything one step's prompt is made of.
#[derive(Clone, Copy, Debug)]
pub struct PromptInput<'a> {
  pub project: &'a Project,
  pub task: &'a Task,
  /// `None` in a preview for a run that does not exist yet: the run's variables have no value.
  pub run: Option<RunContext<'a>>,
  pub pipeline: PipelineInfo<'a>,
  pub step: &'a Step,
  pub profile: ProfileInfo<'a>,
  /// The project's instructions a run of this pipeline pinned (or, in a preview, would pin), in
  /// library order. Those that do not reach this agent at the event are left out.
  pub instructions: &'a [InstructionSource<'a>],
  /// The skills the agent gets, if any, listed at [`InjectionEvent::Start`].
  pub skills: &'a [SkillListing<'a>],
  /// What the agent is told about the code index at either event; `None` tells it nothing (the
  /// project has it off, or it could not be built).
  pub code_index: Option<CodeIndexPrompt<'a>>,
  /// Outputs of the steps `step` depends on, in pipeline order.
  pub dependencies: &'a [DependencyOutput<'a>],
  /// The step's plan a reviewer sent back, when this invocation revises it.
  pub revision: Option<PlanRevision<'a>>,
  /// People's messages this invocation delivers, if any.
  pub messages: Option<DeliveredMessages<'a>>,
}

impl PromptInput<'_> {
  /// The prompt text sent to the agent CLI at `event`, with where each part came from.
  pub fn assemble(&self, event: InjectionEvent) -> Result<AssembledPrompt, PromptAssemblyError> {
    self.compose(event).into_prompt()
  }

  /// Every section of the prompt at `event`, and the instructions left out and why.
  pub fn compose(&self, event: InjectionEvent) -> Composition {
    Composition::new(self, event)
  }

  /// The value of `variable` for this prompt; `None` when it has none.
  pub fn value(&self, variable: Variable) -> Option<String> {
    let details = self.task.details();
    let profile = self.profile.body;
    let run = self.run.map(|context| context.run);
    Some(match variable {
      Variable::TaskId => self.task.key().to_string(),
      Variable::TaskTitle => details.title.as_str().to_owned(),
      Variable::TaskDescription => details.brief.clone(),
      Variable::TaskAcceptanceCriteria => composition::criteria(&details.acceptance_criteria),
      Variable::ProjectId => self.project.id().to_string(),
      Variable::ProjectName => self.project.name().as_str().to_owned(),
      Variable::AgentId => self.profile.id.to_string(),
      Variable::AgentName | Variable::AgentRole => profile.role().as_str().to_owned(),
      Variable::AgentModel => profile.model().as_str().to_owned(),
      Variable::PipelineId => self.pipeline.id.to_string(),
      Variable::PipelineName => self.pipeline.name.to_owned(),
      // An integer, as the catalog types it.
      Variable::PipelineVersion => self.pipeline.version.get().to_string(),
      Variable::RunId => run?.id().to_string(),
      Variable::RunAttempt => self.run?.attempt?.to_string(),
      Variable::RunBranch => run?.branch().as_str().to_owned(),
      Variable::RunBaseRevision => run?.base().as_str().to_owned(),
      // The latest checkpoint as the prompt is made, i.e. when its invocation starts: none
      // before the run's first checkpoint commit.
      Variable::RunRevision => run?.last_checkpoint()?.revision().as_str().to_owned(),
      Variable::StepId => self.step.key.as_str().to_owned(),
      Variable::StepName => self.step.name.as_str().to_owned(),
      Variable::RunBudget => run?.budget().to_string(),
      // Not known to a run yet: tasks have no attachments, the project records no repository
      // or default branch, and creation times have no agreed text form.
      Variable::TaskAttachments
      | Variable::TaskCreatedAt
      | Variable::ProjectRepository
      | Variable::ProjectDefaultBranch => return None,
    })
  }
}
