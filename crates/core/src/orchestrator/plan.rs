//! The Orchestrator's plan-only mode (ORC-336, `docs/design/side-question-and-plan-mode.md`, "Plan-only mode"): a
//! conversation whose switch is on sends each message as a turn that plans ([`TurnMode::Plans`]): the Orchestrator may
//! read anything but takes no action, and answers with a plan ([`ChatPlan`]) of what it would do. The person approves
//! it, which carries it out as an ordinary turn ([`TurnMode::CarriesOut`]), changes it, which asks for a new plan in
//! its place, or dismisses it.

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// The most steps a plan holds.
pub const MAX_STEPS: usize = 20;

/// How the turn of a person's message goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurnMode {
  /// As every turn: the Orchestrator acts through its tools by their rules.
  #[default]
  Acts,
  /// It takes no action: it reads, and answers with a plan, or plainly when nothing needs doing.
  Plans,
  /// It carries out a plan the person approved: the message is that plan, not words of the person's.
  CarriesOut,
}

/// A mode's word the database does not know.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
#[error("no turn mode is named {0}")]
pub struct UnknownMode(pub String);

impl TurnMode {
  /// The word the database keeps; `None` for an ordinary turn.
  pub const fn word(self) -> Option<&'static str> {
    match self {
      Self::Acts => None,
      Self::Plans => Some("plan"),
      Self::CarriesOut => Some("carry_out"),
    }
  }

  /// The mode the database keeps as `word`.
  pub fn of_word(word: Option<&str>) -> Result<Self, UnknownMode> {
    match word {
      None => Ok(Self::Acts),
      Some("plan") => Ok(Self::Plans),
      Some("carry_out") => Ok(Self::CarriesOut),
      Some(other) => Err(UnknownMode(other.to_owned())),
    }
  }
}

/// What kind of action a step of a plan is, which its icon shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
  /// A run started, continued, paused or merged.
  Run,
  /// A task or a draft created or changed.
  Draft,
  /// A setting of the project changed.
  Setting,
  /// Anything else: a message to an agent, an answer, a publication.
  Other,
}

/// One step of a plan: what will happen, and what it acts on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
  kind: StepKind,
  action: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  target: Option<String>,
}

impl PlanStep {
  /// A step of `kind` doing `action` on `target`; their words without the whitespace around them, a blank target
  /// none. `None` for a step that says nothing.
  pub fn new(kind: StepKind, action: &str, target: Option<&str>) -> Option<Self> {
    let action = action.trim();
    if action.is_empty() {
      return None;
    }
    Some(Self {
      kind,
      action: action.to_owned(),
      target: said(target),
    })
  }

  pub const fn kind(&self) -> StepKind {
    self.kind
  }

  pub fn action(&self) -> &str {
    &self.action
  }

  pub fn target(&self) -> Option<&str> {
    self.target.as_deref()
  }
}

/// Where a plan stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PlanState {
  /// It waits for the person: nothing of it was done.
  Waits,
  /// The person approved it at `at`: a turn carries it out.
  Approved { at: Timestamp },
  /// A newer plan of the conversation took its place at `at`, as the person asked by changing it.
  Replaced { at: Timestamp },
  /// The person dismissed it at `at`: nothing was done.
  Dismissed { at: Timestamp },
}

/// Why a plan cannot be kept.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
  #[error("a plan has at least one step")]
  NoSteps,
  #[error("a plan has at most {MAX_STEPS} steps")]
  TooManySteps,
}

/// A plan the Orchestrator answered with in a turn that plans: its steps in order, what it leaves out and why, and what
/// it uses (runs, drafts, the connection, an estimate of tokens).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPlan {
  steps: Vec<PlanStep>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  left_out: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  uses: Option<String>,
  state: PlanState,
}

impl ChatPlan {
  /// A plan of `steps` that waits for the person; blank facts are none.
  pub fn new(steps: Vec<PlanStep>, left_out: Option<&str>, uses: Option<&str>) -> Result<Self, PlanError> {
    if steps.is_empty() {
      return Err(PlanError::NoSteps);
    }
    if steps.len() > MAX_STEPS {
      return Err(PlanError::TooManySteps);
    }
    Ok(Self {
      steps,
      left_out: said(left_out),
      uses: said(uses),
      state: PlanState::Waits,
    })
  }

  pub fn steps(&self) -> &[PlanStep] {
    &self.steps
  }

  pub fn left_out(&self) -> Option<&str> {
    self.left_out.as_deref()
  }

  pub fn uses(&self) -> Option<&str> {
    self.uses.as_deref()
  }

  pub const fn state(&self) -> PlanState {
    self.state
  }

  /// Whether it waits for the person.
  pub const fn waits(&self) -> bool {
    matches!(self.state, PlanState::Waits)
  }

  /// The plan as it stands at `state`.
  #[must_use]
  pub fn settled(mut self, state: PlanState) -> Self {
    self.state = state;
    self
  }
}

/// `words` without the whitespace around them; `None` when they say nothing.
fn said(words: Option<&str>) -> Option<String> {
  words
    .map(str::trim)
    .filter(|words| !words.is_empty())
    .map(str::to_owned)
}

/// The prompt of a turn that plans, for the person's message `words`: take no action, and answer with a plan.
pub fn plan_prompt(words: &str) -> String {
  format!(
    "[Plan only] The person sent this message in plan-only mode: take no action in this turn. You may read anything: \
     the tools that only read, and the project's files. Call no tool that changes something: no task, draft, run, \
     merge, setting, message or answer to an agent, and no card for the person; Orchester refuses each such call. \
     Answer with a plan of the actions you would take: call propose_plan once, with its steps in order, what you \
     leave out and why, and what it uses; then say in a sentence that nothing has started and the person can approve \
     it or tell you what to change. When nothing needs doing, answer plainly and propose no plan. When the person \
     changes a plan of yours, propose the whole changed plan again.\n\
     \n\
     The person's message:\n{words}"
  )
}

/// The prompt of the turn that carries out `plan`, which the person approved.
pub fn carry_out_prompt(plan: &ChatPlan) -> String {
  let steps: Vec<String> = plan
    .steps()
    .iter()
    .enumerate()
    .map(|(index, step)| match step.target() {
      Some(target) => format!("{}. {} ({target})", index + 1, step.action()),
      None => format!("{}. {}", index + 1, step.action()),
    })
    .collect();
  let left_out = plan
    .left_out()
    .map(|left_out| format!("\nLeft out: {left_out}"))
    .unwrap_or_default();
  format!(
    "[Orchester] The person approved your plan: carry it out now, step by step in its order, and do only what it \
     lists; any other step needs a new plan. When a step fails, stop and say why; do not go on with the rest. What \
     only the person decides still waits for them as a card (a start the project does not let you take, a \
     publication or a merge their switches do not let you do): tell them so. Then say what you did, with the links.\n\
     \n\
     The plan:\n{steps}{left_out}",
    steps = steps.join("\n"),
  )
}
