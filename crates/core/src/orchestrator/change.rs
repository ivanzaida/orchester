//! A change the Orchestrator's agent proposed that only a person carries out, unless the person's switch lets the chat
//! carry it out itself (`docs/design/orchestrator-toolset.md`, "Decisions" 3 to 5): publishing a draft, canceling a
//! run, merging one, answering an agent's question, sending a run's agents a message or continuing a run that waits
//! for a person's decision. It holds what the person reads before *Apply* and what *Apply* would change; a draft saved
//! again, a version published meanwhile or a run with newer work needs a fresh proposal.

use serde::{Deserialize, Serialize};

use super::wait::StopReason;
use crate::{
  id::{ResourceId, RunId},
  resource::StepKey,
  run::Run,
  time::Timestamp,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedChange {
  /// What it changes as people read it: the draft's name, or the title of the run's task.
  pub subject: String,
  pub change: Change,
}

/// What a proposed change would do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
  /// Publish the draft of `resource`, of kind `of`, as version `version`: the draft as it was saved at `saved`.
  Publish {
    of: Publishing,
    resource: ResourceId,
    version: u32,
    saved: Timestamp,
  },
  /// Cancel run `run`, `#number`.
  Cancel { run: RunId, number: u32 },
  /// Merge run `run`, `#number`, holding what its last checkpoint `checkpoint` (a commit) holds, into the branch
  /// `into` it started from, when it names one.
  Merge {
    run: RunId,
    number: u32,
    checkpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    into: Option<String>,
  },
  /// Answer the question with suggested answers `request` (its id) an agent of run `run`, `#number`, waits with: the
  /// labels `choices` of the options picked and/or `text`, in the Orchestrator's words.
  Answer {
    run: RunId,
    number: u32,
    request: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    choices: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
  },
  /// Send `text` to the agents of run `run`, `#number`, its task's latest, as the task's composer sends it in the
  /// run's state: the answer to an agent's question in the conversation among others; `now` to an agent at work
  /// whose connection takes it mid-turn, else after its turn. `then` is what sending it does to a run no agent of
  /// which waits or works: it follows the run up, continues or resumes it, or runs the task again.
  Message {
    run: RunId,
    number: u32,
    text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    now: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    then: Option<MessageThen>,
    /// What sending it does in the run's state when it was checked (`send_message`'s `action`): it is sent only as
    /// that, and refused once the run moved on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sent_as: Option<String>,
  },
  /// Pause run `run`, `#number`, at its next checkpoint.
  Pause { run: RunId, number: u32 },
  /// Resume run `run`, `#number`, which a pause stopped.
  Resume { run: RunId, number: u32 },
  /// Let new starts go on: of the reviewed group task `task` (its key) belongs to, else of the whole project.
  Release {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task: Option<String>,
  },
  /// Continue run `run`, `#number`, which stopped in its attempt `attempt` and waits for a person's decision, at the
  /// step `step` for `why` when they are known, with `choice`, as `continue_run` continues it.
  Continue {
    run: RunId,
    number: u32,
    attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    step: Option<StepName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    why: Option<StopReason>,
    choice: ContinueChoice,
  },
}

/// A step of a run's pipeline as people read it: its place in the pipeline, from 1, and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepName {
  pub place: u32,
  pub name: String,
}

impl StepName {
  /// `run`'s step `key` as people read it; `None` for a step its pipeline does not list.
  pub fn in_run(run: &Run, key: &StepKey) -> Option<Self> {
    let steps = run.snapshot().body().steps();
    let index = steps.iter().position(|step| step.key == *key)?;
    Some(Self {
      place: u32::try_from(index + 1).ok()?,
      name: steps[index].name.as_str().to_owned(),
    })
  }
}

/// How a run that waits for a person's decision goes on, of what the chat may propose: raising limits or budgets,
/// resuming after the daily budget and accepting a step stay the person's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContinueChoice {
  /// One more retry of the stopped step (or, for a step that waits unverified or unchanged, that step again).
  AllowRetry,
  /// Back to the earlier agent step `step` (its key), `to` as people read it while the run's pipeline lists it, with
  /// `note` for its agent.
  ReturnTo {
    step: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<StepName>,
    note: String,
  },
}

/// What a message to a run none of whose agents waits or works does to it, as the task's composer sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageThen {
  /// The run is ready for review: a follow-up attempt works on the message.
  FollowUp,
  /// The run failed or waits for a decision: it continues with the message's instructions.
  Continue,
  /// A pause stopped the run: it resumes, and its agent reads the message.
  Resume,
  /// The run was interrupted: it continues from its last checkpoint, and its agent reads the message.
  ContinueInterrupted,
  /// The run stopped at a limit of its own: the message waits until a person raises it.
  RaiseLimit,
  /// The run ended: the task runs again, and its next run's first agent reads the message.
  RunAgain,
}

impl Change {
  /// The run it acts on; `None` for a publication or a release of new starts.
  pub const fn run(&self) -> Option<RunId> {
    match self {
      Self::Publish { .. } | Self::Release { .. } => None,
      Self::Cancel { run, .. }
      | Self::Merge { run, .. }
      | Self::Answer { run, .. }
      | Self::Message { run, .. }
      | Self::Continue { run, .. }
      | Self::Pause { run, .. }
      | Self::Resume { run, .. } => Some(*run),
    }
  }

  /// Whether it carries a run's agents on: an answer to a question, a message, a decision on how a stopped run
  /// continues, or a resume. What the run does next follows from it.
  pub const fn carries_run_on(&self) -> bool {
    matches!(
      self,
      Self::Answer { .. } | Self::Message { .. } | Self::Continue { .. } | Self::Resume { .. }
    )
  }

  /// Whether it moves a run no agent of which waits or works: a message that follows it up, continues or resumes
  /// it, or runs its task again. Such a message is the person's to send, never the chat's by itself.
  pub const fn moves_run(&self) -> bool {
    matches!(self, Self::Message { then: Some(_), .. })
  }

  /// Whether it runs the task again: its next run follows from it.
  pub const fn runs_again(&self) -> bool {
    matches!(
      self,
      Self::Message {
        then: Some(MessageThen::RunAgain),
        ..
      }
    )
  }
}

/// What kind of draft a publication publishes, and so which of Orchester's tools publishes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Publishing {
  Pipeline,
  Profile,
  Instruction,
  Skill,
  Tool,
}

impl Publishing {
  pub const ALL: [Self; 5] = [
    Self::Pipeline,
    Self::Profile,
    Self::Instruction,
    Self::Skill,
    Self::Tool,
  ];

  /// The tool that publishes it (`publish_pipeline`, …).
  pub const fn tool(self) -> &'static str {
    match self {
      Self::Pipeline => "publish_pipeline",
      Self::Profile => "publish_profile",
      Self::Instruction => "publish_instruction",
      Self::Skill => "publish_skill",
      Self::Tool => "publish_tool",
    }
  }

  /// The argument that tool names the draft's resource by (`pipeline`, …).
  pub const fn argument(self) -> &'static str {
    match self {
      Self::Pipeline => "pipeline",
      Self::Profile => "profile",
      Self::Instruction => "instruction",
      Self::Skill => "skill",
      Self::Tool => "tool",
    }
  }

  /// Whether publishing a draft of this kind can widen what agents may do, so that its tool refuses it as
  /// `person_only` while *Let automation change agent permissions* is off. Instructions and skills reach agents'
  /// prompts only; a kind that gains such a rule must say so here and be judged by the chat's widening rules.
  pub const fn can_widen(self) -> bool {
    match self {
      Self::Pipeline | Self::Profile | Self::Tool => true,
      Self::Instruction | Self::Skill => false,
    }
  }

  /// The kind published by tool `name`; `None` for a tool that publishes nothing.
  pub fn of_tool(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.tool() == name)
  }
}
