//! What the composer says in each state of the run (`docs/design/orchester-workspace.md`, "Composer
//! (approved …)", `YJfPo`): what it answers or changes, above the box; its placeholder (*Message
//! Builder…*); each recipient as its name and what it does with a message (*Builder · working*,
//! *Planner · resumes its session*); the send button's tooltip, which carries what sending does; and
//! the quiet line under the box, only while it changes the outcome (the message queues, a follow-up
//! resumes the session, a failed run continues, the run has not started).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{conversation::Recipient, id::TaskId};

use super::super::{divider::step_label, nav};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::{
    conversation::{
      Author, Conversation, PanelState, RecipientChoice,
      composer::{ComposerMode, Queued},
    },
    detail::TaskDetail,
  },
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// What the field says while it is empty: whom the message is for, or what it is.
pub(super) fn placeholder(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Arc<str> {
  let to_agent = |agent: &str| ctx.t_args("runs.conversation.placeholder.agent", [("agent", agent)]);
  match mode {
    ComposerMode::Steer(working) => to_agent(&working.agent),
    ComposerMode::Queue(Queued::NextTurn(Some(agent))) => to_agent(agent),
    ComposerMode::Queue(Queued::ToolStep | Queued::NextTurn(None) | Queued::Suspended | Queued::Decision) => {
      ctx.t("runs.conversation.placeholder.run")
    }
    ComposerMode::Answer { agent, .. } => ctx.t_args("runs.conversation.placeholder.answer", [("agent", &**agent)]),
    ComposerMode::PlanChanges { .. } => ctx.t("runs.conversation.placeholder.planChanges"),
    ComposerMode::FollowUp => ctx.t("runs.conversation.placeholder.followUp"),
    ComposerMode::Continue => match conversation.next_agent.as_deref() {
      Some(agent) => ctx.t_args("runs.conversation.placeholder.continue", [("agent", agent)]),
      None => ctx.t("runs.conversation.placeholder.continueUnnamed"),
    },
    ComposerMode::NotStarted => match conversation.first_agent.as_deref() {
      Some(first) => ctx.t_args("runs.conversation.placeholder.firstAgent", [("agent", first)]),
      None => ctx.t("conversation.composer.placeholder"),
    },
    ComposerMode::Queue(Queued::Checkpoint) | ComposerMode::Closed => ctx.t("conversation.composer.placeholder"),
  }
}

/// The state's primary action, as the send button's tooltip names it.
pub(super) fn primary_label(ctx: &Ctx, mode: &ComposerMode) -> Arc<str> {
  ctx.t(match mode {
    ComposerMode::Steer(working) if working.mid_turn => "runs.conversation.composer.sendNow",
    ComposerMode::Steer(_) => "runs.conversation.composer.sendAfterTurn",
    ComposerMode::Queue(_) | ComposerMode::Closed => "conversation.composer.send",
    ComposerMode::Answer { .. } => "runs.conversation.composer.sendAnswer",
    ComposerMode::PlanChanges { .. } => "runs.plan.requestChanges",
    ComposerMode::FollowUp => "runs.conversation.composer.sendFollowUp",
    ComposerMode::Continue => "runs.conversation.composer.continue",
    ComposerMode::NotStarted => "runs.conversation.composer.sendWhenStarts",
  })
}

/// One recipient as the selector shows it in `mode`: its name, and what it does with a message
/// (`opOGv`, the recipient menu of `l7cEtA`).
pub(super) fn recipient_words(
  ctx: &Ctx,
  conversation: &Conversation,
  mode: &ComposerMode,
  choice: &RecipientChoice,
) -> (Arc<str>, Arc<str>) {
  let Some(agent) = choice.agent.clone() else {
    return match (mode, conversation.first_agent.clone()) {
      (ComposerMode::NotStarted, Some(first)) => (first, ctx.t("runs.conversation.recipient.startsFirst")),
      _ => (
        ctx.t("runs.conversation.recipient.orchestrator"),
        ctx.t("runs.conversation.recipient.thisTask"),
      ),
    };
  };
  let working = matches!(choice.recipient, Recipient::Agent(profile)
    if conversation.working.iter().any(|working| working.profile == profile));
  let key = match mode {
    _ if working => "runs.conversation.recipient.working",
    ComposerMode::Answer { .. } => "runs.conversation.recipient.asking",
    ComposerMode::PlanChanges { .. } => "runs.conversation.recipient.planWaits",
    ComposerMode::FollowUp => "runs.conversation.recipient.resumes",
    ComposerMode::Continue => "runs.conversation.recipient.continues",
    ComposerMode::Queue(Queued::Suspended) => match conversation.state {
      PanelState::Stopped => "runs.conversation.recipient.stopped",
      PanelState::Interrupted => "runs.conversation.recipient.interrupted",
      _ => "runs.conversation.recipient.paused",
    },
    ComposerMode::Queue(Queued::Decision) => "runs.conversation.recipient.deciding",
    _ => "runs.conversation.recipient.notRunning",
  };
  (agent, ctx.t(key))
}

/// What sending does now, said by the send button's tooltip (`k565Es` in `l7cEtA`): *Send now ·
/// Enter. Builder gets it at its next tool call. Ctrl+Enter sends after this turn.*
pub(super) fn tip(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Arc<str> {
  if let ComposerMode::Steer(working) = mode
    && working.mid_turn
  {
    let key = if cfg!(target_os = "macos") {
      "runs.conversation.tip.steerMac"
    } else {
      "runs.conversation.tip.steer"
    };
    return ctx.t_args(key, [("agent", &*working.agent)]);
  }
  ctx.t_args(
    "runs.conversation.tip.send",
    [
      ("action", &*primary_label(ctx, mode)),
      ("hint", &*hint(ctx, mode, conversation)),
    ],
  )
}

/// The quiet line under the box, while what it says changes the outcome (`okcPt` in `l7cEtA`,
/// `Alok8` in `cHDF1`): the message queues, a follow-up resumes the session, a failed run
/// continues, the run has not started.
pub(super) fn quiet(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Option<Arc<str>> {
  match mode {
    ComposerMode::Queue(_) | ComposerMode::FollowUp | ComposerMode::Continue | ComposerMode::NotStarted => {
      Some(hint(ctx, mode, conversation))
    }
    ComposerMode::Steer(_) | ComposerMode::Answer { .. } | ComposerMode::PlanChanges { .. } | ComposerMode::Closed => {
      None
    }
  }
}

/// What sending does now.
pub(super) fn hint(ctx: &Ctx, mode: &ComposerMode, conversation: &Conversation) -> Arc<str> {
  match mode {
    ComposerMode::Steer(working) => {
      let key = match (working.mid_turn, cfg!(target_os = "macos")) {
        (true, false) => "runs.conversation.hint.steer",
        (true, true) => "runs.conversation.hint.steerMac",
        (false, _) => "runs.conversation.hint.steerUnavailable",
      };
      ctx.t_args(key, [("agent", &*working.agent)])
    }
    ComposerMode::Queue(queued) => {
      taken_as_answer(ctx, queued, conversation).unwrap_or_else(|| queue_hint(ctx, queued, conversation.state))
    }
    ComposerMode::Answer { agent, .. } => ctx.t_args("conversation.explanation.answer", [("agent", &**agent)]),
    ComposerMode::PlanChanges { agent, .. } => ctx.t_args("runs.conversation.hint.planChanges", [("agent", &**agent)]),
    ComposerMode::FollowUp => match conversation.next_agent.as_deref() {
      Some(agent) => ctx.t_args("conversation.explanation.ready", [("agent", agent)]),
      None => ctx.t("conversation.explanation.readyNoAgent"),
    },
    ComposerMode::Continue => ctx.t("conversation.explanation.failed"),
    ComposerMode::NotStarted => ctx.t("conversation.explanation.notStarted"),
    ComposerMode::Closed => ctx.t(match conversation.state {
      PanelState::Merged => "conversation.explanation.merged",
      PanelState::WorktreeRemoved | PanelState::FailedWorktreeRemoved => "conversation.explanation.worktreeRemoved",
      PanelState::FailedNoAgent => "conversation.explanation.failedNoAgent",
      _ => "conversation.explanation.ended",
    }),
  }
}

/// A message written while a question waits, its answer mode left, still answers it when it reaches
/// the agent that asked: written to that agent or to Orchestrator (the runner resumes the asking step
/// with any message that reaches its agent).
fn taken_as_answer(ctx: &Ctx, queued: &Queued, conversation: &Conversation) -> Option<Arc<str>> {
  let question = conversation.open_question()?;
  let Author::Agent(asker) = &question.author else {
    return None;
  };
  let reaches = match queued {
    Queued::NextTurn(Some(agent)) => agent == asker,
    Queued::Checkpoint => true,
    Queued::NextTurn(None) | Queued::ToolStep | Queued::Suspended | Queued::Decision => false,
  };
  reaches.then(|| ctx.t_args("runs.conversation.hint.takenAsAnswer", [("agent", &**asker)]))
}

fn queue_hint(ctx: &Ctx, queued: &Queued, state: PanelState) -> Arc<str> {
  match queued {
    Queued::ToolStep => ctx.t("runs.conversation.hint.toolStep"),
    Queued::NextTurn(Some(agent)) => ctx.t_args("runs.conversation.hint.nextTurn", [("agent", &**agent)]),
    Queued::NextTurn(None) => ctx.t("runs.conversation.hint.nextTurnUnnamed"),
    Queued::Checkpoint => ctx.t("conversation.explanation.checkpoint"),
    Queued::Decision => ctx.t("conversation.explanation.deciding"),
    Queued::Suspended => ctx.t(match state {
      PanelState::Paused => "conversation.explanation.paused",
      PanelState::Stopped => "conversation.explanation.stopped",
      _ => "conversation.explanation.queued",
    }),
  }
}

/// Above the box (`po1Yr` in `K1oNwn`, `l7cEtA`): *Answering Builder's question from 12:09*,
/// *Changes to plan v1 · sent to Planner* with × to `leave` that mode, *Continue 03 Implement with
/// your instructions · attempt 3*; *Waiting for your decision* with the way to it, on the task's
/// Summary.
pub(super) fn context_line(
  ctx: &mut Ctx,
  (mode, detail): (&ComposerMode, &TaskDetail),
  leave: Option<Arc<dyn Fn() + Send + Sync>>,
) -> Option<Element> {
  let (glyph, text) = match mode {
    ComposerMode::Queue(Queued::Decision) => return Some(decision_line(ctx, detail.task.id())),
    ComposerMode::Answer { agent, asked_at, .. } => (
      Icon::CornerDownRight,
      ctx.t_args(
        "runs.conversation.composer.answering",
        [("agent", &**agent), ("time", display::clock(*asked_at).as_str())],
      ),
    ),
    ComposerMode::PlanChanges { agent, version } => (
      Icon::Pencil,
      ctx.t_args(
        "runs.conversation.composer.planChanges",
        [("version", version.to_string().as_str()), ("agent", &**agent)],
      ),
    ),
    ComposerMode::Continue => (Icon::Play, continue_text(ctx, detail)?),
    _ => return None,
  };
  let leave = leave.map(|leave| {
    pressable(surface().id("conversation-composer-leave"), move || leave()).child(icon(
      Icon::X,
      IconSize::Small,
      PaletteColor::TextMuted,
    ))
  });
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Md)
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
      .child(
        Text::new(&text)
          .id("conversation-composer-context")
          .variant(TypographyStyle::Caption),
      )
      .child(Spacer::new().flex(1.0))
      .with_children(leave)
      .into(),
  )
}

/// *Continue 03 Implement with your instructions · attempt 3*: the step the failed run stopped at
/// and the attempt the message starts.
fn continue_text(ctx: &Ctx, detail: &TaskDetail) -> Option<Arc<str>> {
  let run = detail.run.as_ref()?;
  let step = run.current_step()?;
  let attempt = run.run.attempts().len() + 1;
  Some(ctx.t_args(
    "runs.conversation.composer.continueContext",
    [
      ("step", &*step_label(ctx, step.number, &step.name)),
      ("attempt", attempt.to_string().as_str()),
    ],
  ))
}

/// The run waits for the person's decision after its retries: what the composer's messages wait for,
/// and *Open the decision →*, which opens the Summary with the decision panel.
fn decision_line(ctx: &mut Ctx, task: TaskId) -> Element {
  let link = nav(ctx)
    .link(Page::Task(task, TaskPageTab::Summary))
    .on(surface().id("conversation-open-decision"))
    .child(Text::new(&ctx.t("conversation.openDecision")).variant(TypographyStyle::Link));
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("runs.conversation.composer.deciding"))
        .id("conversation-status")
        .variant(TypographyStyle::Caption),
    )
    .child(link)
    .into()
}
