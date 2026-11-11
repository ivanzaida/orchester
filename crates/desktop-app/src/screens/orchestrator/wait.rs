//! A wait of an agent of the project in the Orchestrator's conversation (`2 · Agent wait` `GIsJh` in `Orchestrator
//! cards · redesign` `zuvUi`; `docs/design/orchestrator-toolset.md`, "Decisions" 5): the event line of what the agent
//! waits for (a question, a reply, a plan or a tool request to decide), the task and the run, when it was noticed and
//! *Open ↗* its card
//! on the task's Conversation, with a status chip: *Waits for you*, *Orchestrator is reading* (the loader), *Answered
//! by the Orchestrator*, *Answered by you*, or *No longer waits* once it was dealt with elsewhere (the workspace reads
//! which waits still stand). What the Orchestrator said or did with it nests under it as a quote
//! (`super::timeline`). Whether *Let the Orchestrator drive* answers it is the chip's tooltip. Deciding a plan stays
//! the person's: its chip says it waits for them, and the Orchestrator's reading nests as its recommendation. A tool
//! request is read and decided by the Orchestrator while it drives (2026-11-11, [`super::approval`]): *Allowed by the
//! Orchestrator* or *Denied by the Orchestrator* once its `decide_approval` was done, how the person answered it once
//! they did, else it waits for them on its card.
//! The end of a task's run is a wait of its own kind, with its own line ([`super::run_end`]).

pub(super) mod approval;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ActionOutcome, AgentWait, TurnState, WaitKind, change::Change},
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::requests::ChatRequest};

use super::{
  line::{Chip, ChipTone, Line, LineLink, Tone},
  turns::Part,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{icon::Icon, pressable::Press},
};

/// Where a wait stands, as its chip says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
  /// It waits for the person.
  Waits,
  /// The Orchestrator reads it, now or after the turn under way.
  Reading,
  /// The Orchestrator answered the agent itself.
  Answered,
  /// The person answered it, applying what the Orchestrator proposed.
  AnsweredByYou,
  /// It was dealt with elsewhere, on its task's page: its agent no longer waits.
  Gone,
  /// A tool request the Orchestrator allowed once.
  Allowed,
  /// A tool request the Orchestrator denied.
  Denied,
  /// A tool request the person answered, as the chip's words (`key`) say.
  ByYou(&'static str),
}

impl Status {
  /// Where `wait` stands, by whether it still `stands` (its agent still waits), the turn it started (`turn`, `None`
  /// for one that started none) and what that turn's answer did (`answer`).
  pub(super) fn of(
    wait: &AgentWait,
    (stands, request): (bool, Option<&ChatRequest>),
    turn: Option<&TurnState>,
    answer: &[Part],
  ) -> Self {
    let thinking = matches!(turn, Some(TurnState::Thinking));
    if wait.kind == WaitKind::Plan {
      return if stands { Self::Waits } else { Self::Gone };
    }
    if wait.kind == WaitKind::Approval {
      return Self::of_approval(wait, (stands, request), turn, answer);
    }
    if thinking {
      return Self::Reading;
    }
    let answered = answer.iter().find_map(|part| match part {
      Part::Action { action, .. }
        if matches!(
          action.change.as_ref().map(|change| &change.change),
          Some(Change::Answer { .. } | Change::Message { .. })
        ) =>
      {
        match action.outcome {
          ActionOutcome::Done => Some(Self::Answered),
          ActionOutcome::Applied => Some(Self::AnsweredByYou),
          _ => None,
        }
      }
      _ => None,
    });
    match answered {
      Some(answered) => answered,
      // The Orchestrator takes up only a wait that still waits.
      None if !stands => Self::Gone,
      None if turn.is_none() && wait.automatic => Self::Reading,
      None => Self::Waits,
    }
  }

  /// Where a tool request (`request`) stands: allowed or denied by the Orchestrator's `decide_approval` in its turn,
  /// answered by the person, read while that turn thinks or waits to start, else the person's.
  fn of_approval(
    wait: &AgentWait,
    (stands, request): (bool, Option<&ChatRequest>),
    turn: Option<&TurnState>,
    answer: &[Part],
  ) -> Self {
    if let Some(own) = approval::own_answer(answer) {
      return if own.allowed { Self::Allowed } else { Self::Denied };
    }
    if let Some(key) = request
      .and_then(|request| request.decided.as_ref())
      .and_then(approval::by_you)
    {
      return Self::ByYou(key);
    }
    match (stands, turn) {
      (false, _) => Self::Gone,
      (true, Some(TurnState::Thinking)) => Self::Reading,
      (true, None) if wait.automatic => Self::Reading,
      (true, _) => Self::Waits,
    }
  }

  /// The name tools read.
  pub(super) const fn name(self) -> &'static str {
    match self {
      Self::Waits => "waits",
      Self::Reading => "reading",
      Self::Answered => "answered",
      Self::AnsweredByYou => "answered_by_you",
      Self::Gone => "gone",
      Self::Allowed => "allowed",
      Self::Denied => "denied",
      Self::ByYou(_) => "decided_by_you",
    }
  }

  fn chip(self, ctx: &Ctx) -> Chip {
    let (key, tone) = match self {
      Self::Waits => ("orchestrator.wait.status.waits", ChipTone::Warning),
      Self::Reading => ("orchestrator.wait.status.reading", ChipTone::Working),
      Self::Answered => ("orchestrator.wait.status.answered", ChipTone::Success),
      Self::AnsweredByYou => ("orchestrator.wait.status.answeredByYou", ChipTone::Neutral),
      Self::Gone => ("orchestrator.wait.status.gone", ChipTone::Neutral),
      Self::Allowed => ("orchestrator.wait.status.allowed", ChipTone::Success),
      Self::Denied => ("orchestrator.wait.status.denied", ChipTone::Danger),
      Self::ByYou(key) => (key, ChipTone::Neutral),
    };
    Chip::new(ctx.t(key), tone)
  }
}

/// The line of `wait` (not a run's end), kept at `at`, standing at `status`, which started an Orchestrator turn by
/// itself when `turned`; in project `project`; `id` names it.
pub(super) fn line(
  ctx: &mut Ctx,
  id: &str,
  (wait, at, turned): (&AgentWait, Option<Timestamp>, bool),
  (status, project): (Status, ProjectId),
) -> Element {
  let named = wait.agent.as_deref().or(wait.step.as_deref());
  let (glyph, key) = look(wait.kind, named.is_some());
  let what = ctx.t_args(key, [("agent", named.unwrap_or_default())]);
  // A wait dealt with elsewhere says so, and is no longer a warning.
  let (tip, tone) = if status == Status::Gone {
    ("orchestrator.wait.effectGone", Tone::Plain)
  } else {
    (effect(wait, turned), Tone::Warning)
  };
  let chip = status.chip(ctx).tip(ctx.t(tip));
  Line::new(id, (glyph, tone), what)
    .outcome(kind_name(wait.kind))
    .status(status.name())
    .target(Some(run_target(ctx, wait)))
    .chip(Some(chip))
    .time(at.map(display::clock))
    .link(open_card(ctx, project, &wait.task))
    .render(ctx)
}

/// The task and the run a wait or a run's end is about: `KON-13 · #9`.
pub(super) fn run_target(ctx: &Ctx, wait: &AgentWait) -> Arc<str> {
  ctx.t_args(
    "orchestrator.line.run",
    [("task", wait.task.as_str()), ("run", &wait.number.to_string())],
  )
}

/// The icon of a wait of `kind` and the key of what it says, naming its agent when it is known.
pub(super) const fn look(kind: WaitKind, named: bool) -> (Icon, &'static str) {
  match (kind, named) {
    (WaitKind::Question, true) => (Icon::CircleHelp, "orchestrator.wait.what.question"),
    (WaitKind::Question, false) => (Icon::CircleHelp, "orchestrator.wait.what.questionAnyone"),
    (WaitKind::Message, true) => (Icon::MessageCircle, "orchestrator.wait.what.message"),
    (WaitKind::Message, false) => (Icon::MessageCircle, "orchestrator.wait.what.messageAnyone"),
    (WaitKind::Approval, true) => (Icon::ShieldAlert, "orchestrator.wait.what.approval"),
    (WaitKind::Approval, false) => (Icon::ShieldAlert, "orchestrator.wait.what.approvalAnyone"),
    (WaitKind::Plan, true) => (Icon::Map, "orchestrator.wait.what.plan"),
    (WaitKind::Plan, false) | (WaitKind::Ended(_) | WaitKind::Event(_), _) => {
      (Icon::Map, "orchestrator.wait.what.planAnyone")
    }
  }
}

const fn kind_name(kind: WaitKind) -> &'static str {
  match kind {
    WaitKind::Question => "question",
    WaitKind::Message => "message",
    WaitKind::Plan => "plan",
    WaitKind::Approval => "approval",
    WaitKind::Ended(_) => "run_end",
    WaitKind::Event(_) => "event",
  }
}

/// What happens with `wait`: the Orchestrator answers it by itself (in the turn it `turned`, or after the turn under
/// way), its own answer raised it so it waits for the person, or the person's switch is off. A tool request the
/// Orchestrator decides only when it is sure.
const fn effect(wait: &AgentWait, turned: bool) -> &'static str {
  match (wait.kind, wait.automatic, turned) {
    (WaitKind::Approval, true, _) => "orchestrator.wait.effectApprovalRead",
    (WaitKind::Approval, false, _) => "orchestrator.wait.effectApproval",
    (WaitKind::Plan, true, _) => "orchestrator.wait.effectPlanRead",
    (WaitKind::Plan, false, _) => "orchestrator.wait.effectPlan",
    (_, true, true) => "orchestrator.wait.effectAnswering",
    (_, true, false) => "orchestrator.wait.effectQueued",
    (_, false, _) => "orchestrator.wait.effectWaits",
  }
}

/// *Open ↗* the task's Conversation, where the wait's card is.
pub(super) fn open_card(ctx: &mut Ctx, project: ProjectId, task: &str) -> Option<LineLink> {
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)));
  Some(LineLink {
    label: ctx.t("orchestrator.line.open"),
    press: Press::Open(navigation.to(route)),
  })
}
