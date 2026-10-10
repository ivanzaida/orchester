//! What the composer says in each state of the run (`docs/design/kontur-handover.md`, "The composer"): its
//! placeholder (*Message Builder…*, *Write your answer…*, *Write to the first agent of the run…*); whom the
//! message goes to and what that agent does with it (*Builder · working*, *Explorer · not started*, *To:
//! Orchestrator · this task*); what the send button does, with its icon (arrow-up hands it over, clock
//! queues it); the send menu's two ways, where the state has two; and the quiet line under the box, only
//! where it changes the outcome (the message queues, a follow-up resumes the session, a run continues,
//! resumes or runs again).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{conversation::Recipient, run::RunLimit};
use orchester_workspace::tasks::conversation::{
  Author, Conversation, WorkingAgent,
  composer::{ComposerMode, Queued},
};

use crate::ui::icon::Icon;

/// What the field says while it is empty: whom the message is for, or what it is.
pub(super) fn placeholder(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Arc<str> {
  let to_agent = |agent: &str| ctx.t_args("composer.placeholder.agent", [("agent", agent)]);
  match mode {
    ComposerMode::Steer(working) => to_agent(&working.agent),
    ComposerMode::Queue(Queued::NextTurn(Some(agent)) | Queued::AnswerAbove(agent)) => to_agent(agent),
    ComposerMode::Queue(Queued::SharedBudget(_)) | ComposerMode::Resume { .. } | ComposerMode::RaiseLimit => {
      match conversation.next_agent.as_deref() {
        Some(agent) => to_agent(agent),
        None => ctx.t("composer.placeholder.run"),
      }
    }
    ComposerMode::Queue(Queued::ToolStep | Queued::NextTurn(None) | Queued::Decision) => {
      ctx.t("composer.placeholder.run")
    }
    ComposerMode::Answer { .. } => ctx.t("composer.placeholder.answer"),
    ComposerMode::PlanChanges { .. } => ctx.t("composer.placeholder.planChanges"),
    ComposerMode::FollowUp => ctx.t("composer.placeholder.followUp"),
    ComposerMode::Continue => match conversation.next_agent.as_deref() {
      Some(agent) => ctx.t_args("composer.placeholder.continue", [("agent", agent)]),
      None => ctx.t("composer.placeholder.continueUnnamed"),
    },
    ComposerMode::NotStarted if conversation.first_agent.is_some() => ctx.t("composer.placeholder.firstAgent"),
    ComposerMode::RunAgain => ctx.t("composer.placeholder.nextRun"),
    ComposerMode::NotStarted | ComposerMode::Queue(Queued::Checkpoint) | ComposerMode::Closed => {
      ctx.t("composer.placeholder.task")
    }
  }
}

/// Whom the message goes to, as the recipient pill says it: the agent and what it does with a message,
/// or Orchestrator for the task. Before the run starts, a message to Orchestrator reaches the run's first
/// agent, which the pill names (`maV9f` in `B67XdS`).
pub(super) fn recipient(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation, to: Recipient) -> Arc<str> {
  let agent = conversation
    .recipients
    .iter()
    .find(|choice| choice.recipient == to)
    .and_then(|choice| choice.agent.clone());
  let (agent, state) = match (agent, mode) {
    (Some(agent), _) => {
      let state = agent_state(mode, conversation, (to, &agent));
      (agent, state)
    }
    (None, ComposerMode::NotStarted) => match conversation.first_agent.clone() {
      Some(first) => (first, "composer.recipient.state.notStarted"),
      None => return ctx.t("composer.recipient.orchestrator"),
    },
    (None, _) => return ctx.t("composer.recipient.orchestrator"),
  };
  ctx.t_args(
    "composer.recipient.agent",
    [("agent", &*agent), ("state", &*ctx.t(state))],
  )
}

/// What `agent`, the recipient `to`, does with a message in `mode`.
fn agent_state(mode: &ComposerMode, conversation: &Conversation, (to, agent): (Recipient, &str)) -> &'static str {
  let working = conversation
    .working
    .iter()
    .any(|working| Recipient::Agent(working.profile) == to);
  match mode {
    ComposerMode::Queue(Queued::AnswerAbove(asker)) if **asker == *agent => "composer.recipient.state.asking",
    _ if working => "composer.recipient.state.working",
    ComposerMode::Answer { .. } => "composer.recipient.state.asking",
    ComposerMode::PlanChanges { .. } => "composer.recipient.state.planWaits",
    ComposerMode::FollowUp => "composer.recipient.state.resumes",
    ComposerMode::Continue => "composer.recipient.state.continues",
    ComposerMode::Resume { interrupted: false } => "composer.recipient.state.paused",
    ComposerMode::Resume { interrupted: true } => "composer.recipient.state.interrupted",
    ComposerMode::RaiseLimit | ComposerMode::Queue(Queued::SharedBudget(_)) => "composer.recipient.state.stopped",
    ComposerMode::Queue(Queued::Decision) => "composer.recipient.state.deciding",
    ComposerMode::RunAgain => "composer.recipient.state.nextRun",
    ComposerMode::NotStarted => "composer.recipient.state.notStarted",
    _ => "composer.recipient.state.notWorking",
  }
}

/// The send button of `mode`: its words, and its icon. While an agent works the button is its icon
/// alone (`ITPkh` in `D1PYAe`), its words in its tooltip.
pub(super) fn primary(ctx: &Ctx, mode: &ComposerMode) -> (Arc<str>, Icon) {
  let (key, glyph) = match mode {
    ComposerMode::Steer(working) if working.mid_turn => ("composer.send.now", Icon::ArrowUp),
    ComposerMode::Steer(_) => ("composer.send.afterTurn", Icon::Clock),
    ComposerMode::Queue(_) | ComposerMode::Closed => ("composer.send.queue", Icon::Clock),
    ComposerMode::Answer { .. } => ("composer.send.answer", Icon::ArrowUp),
    ComposerMode::PlanChanges { .. } => ("composer.send.requestChanges", Icon::Undo2),
    ComposerMode::FollowUp => ("composer.send.followUp", Icon::ArrowUp),
    ComposerMode::Continue => ("composer.send.continue", Icon::Play),
    ComposerMode::Resume { interrupted: false } => ("composer.send.resume", Icon::Play),
    ComposerMode::Resume { interrupted: true } => ("composer.send.continueRun", Icon::Play),
    ComposerMode::RaiseLimit => ("composer.send.raiseLimit", Icon::OctagonPause),
    ComposerMode::NotStarted => ("composer.send.whenStarts", Icon::Clock),
    ComposerMode::RunAgain => ("composer.send.runAgain", Icon::RotateCcw),
  };
  (ctx.t(key), glyph)
}

/// One way of sending in the send menu: what it does, and when its agent gets the message or, where it
/// cannot be chosen, why.
pub(super) struct Way {
  pub(super) label: Arc<str>,
  pub(super) detail: Arc<str>,
  pub(super) unavailable: bool,
}

/// The send menu's two ways in `mode`, the primary first (Enter), then the second (Ctrl/Cmd+Enter):
/// *Send now* and *Send after this turn* while an agent works, *Send now* unavailable with why for a
/// connection that takes no message mid-turn; *Resume and send* and *Send without resuming* while the run
/// is paused or interrupted. `None` where sending has one way.
pub(super) fn menu(ctx: &Ctx, mode: &ComposerMode) -> Option<(Way, Way)> {
  let way = |label: &str, detail: Arc<str>, unavailable: bool| Way {
    label: ctx.t(label),
    detail,
    unavailable,
  };
  match mode {
    ComposerMode::Steer(WorkingAgent { agent, mid_turn, .. }) => {
      let agent = [("agent", &**agent)];
      let after = way(
        "composer.send.afterTurn",
        ctx.t_args("composer.menu.afterTurn", agent),
        false,
      );
      let now = if *mid_turn {
        way("composer.send.now", ctx.t_args("composer.menu.now", agent), false)
      } else {
        way(
          "composer.send.now",
          ctx.t_args("composer.menu.nowUnavailable", agent),
          true,
        )
      };
      Some(if *mid_turn { (now, after) } else { (after, now) })
    }
    ComposerMode::Resume { interrupted } => {
      let (later, detail) = if *interrupted {
        ("composer.send.withoutContinuing", "composer.menu.continueLater")
      } else {
        ("composer.send.withoutResuming", "composer.menu.resumeLater")
      };
      Some((
        Way {
          label: primary(ctx, mode).0,
          detail: ctx.t("composer.menu.resume"),
          unavailable: false,
        },
        way(later, ctx.t(detail), false),
      ))
    }
    _ => None,
  }
}

/// The quiet line under the box (`SpaXX` in `B67XdS`), only where it changes the outcome.
pub(super) fn note(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Option<Arc<str>> {
  let with_agent = |key: &str, agent: &str| ctx.t_args(key, [("agent", agent)]);
  let note = match mode {
    ComposerMode::Queue(queued) => {
      taken_as_answer(ctx, queued, conversation).unwrap_or_else(|| queue_note(ctx, queued))
    }
    ComposerMode::FollowUp => match conversation.next_agent.as_deref() {
      Some(agent) => with_agent("composer.note.followUp", agent),
      None => ctx.t("composer.note.followUpNoAgent"),
    },
    ComposerMode::Continue => ctx.t("composer.note.continue"),
    ComposerMode::Resume { interrupted: false } => ctx.t("composer.note.resume"),
    ComposerMode::Resume { interrupted: true } => ctx.t("composer.note.continueInterrupted"),
    ComposerMode::RaiseLimit => ctx.t("composer.note.raiseLimit"),
    ComposerMode::NotStarted => match conversation.first_agent.as_deref() {
      Some(agent) => with_agent("composer.note.notStarted", agent),
      None => ctx.t("composer.note.notStartedUnnamed"),
    },
    ComposerMode::RunAgain => ctx.t("composer.note.runAgain"),
    ComposerMode::Steer(_) | ComposerMode::Answer { .. } | ComposerMode::PlanChanges { .. } | ComposerMode::Closed => {
      return None;
    }
  };
  Some(note)
}

/// A message written while a question waits, its answer mode left, still answers it when it reaches the
/// agent that asked: written to that agent or to Orchestrator.
fn taken_as_answer(ctx: &Ctx, queued: &Queued, conversation: &Conversation) -> Option<Arc<str>> {
  let question = conversation.open_question()?;
  let Author::Agent(asker) = &question.author else {
    return None;
  };
  let reaches = match queued {
    Queued::NextTurn(Some(agent)) => agent == asker,
    Queued::Checkpoint => true,
    Queued::NextTurn(None) | Queued::ToolStep | Queued::SharedBudget(_) | Queued::Decision | Queued::AnswerAbove(_) => {
      false
    }
  };
  reaches.then(|| ctx.t_args("composer.note.takenAsAnswer", [("agent", &**asker)]))
}

fn queue_note(ctx: &Ctx, queued: &Queued) -> Arc<str> {
  match queued {
    Queued::ToolStep => ctx.t("composer.note.toolStep"),
    Queued::NextTurn(Some(agent)) => ctx.t_args("composer.note.nextTurn", [("agent", &**agent)]),
    Queued::NextTurn(None) => ctx.t("composer.note.nextTurnUnnamed"),
    Queued::Checkpoint => ctx.t("composer.note.checkpoint"),
    Queued::Decision => ctx.t("composer.note.deciding"),
    Queued::SharedBudget(RunLimit::GroupBudget) => ctx.t("composer.note.groupBudget"),
    Queued::SharedBudget(_) => ctx.t("composer.note.dailyBudget"),
    Queued::AnswerAbove(agent) => ctx.t_args("composer.note.answerAbove", [("agent", &**agent)]),
  }
}

/// The keys that send the second way, as this platform names them.
pub(super) fn second_keys(ctx: &Ctx) -> Arc<str> {
  ctx.t(if cfg!(target_os = "macos") {
    "composer.keys.cmdEnter"
  } else {
    "composer.keys.ctrlEnter"
  })
}
