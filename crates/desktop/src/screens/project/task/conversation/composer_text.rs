//! What the composer says in each state of the run: what it answers or changes, above it; its
//! placeholder (*Message Builder…*, `CxrSA`); its primary button; each recipient (*To: Builder ·
//! working*, *To: Builder · resumes its session*, *To: Explorer · first step*) and the line under
//! it that says what sending does now (`docs/design/orchester-workspace.md`, "Composer").

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{conversation::Recipient, id::TaskId};

use super::nav;
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::conversation::{
    Conversation, PanelState, RecipientChoice,
    composer::{ComposerMode, Queued},
  },
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::surface,
  },
};

/// What the field says while it is empty: whom the message is for, or what it is (`CxrSA`,
/// `OlLGD`, `haIEI`, `AzKYQ`, `zxIMy`).
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
    ComposerMode::NotStarted if conversation.first_agent.is_some() => ctx.t("runs.conversation.placeholder.firstAgent"),
    ComposerMode::Queue(Queued::Checkpoint) | ComposerMode::NotStarted | ComposerMode::Closed => {
      ctx.t("conversation.composer.placeholder")
    }
  }
}

/// The state's primary action.
pub(super) fn primary_label(ctx: &Ctx, mode: &ComposerMode) -> Arc<str> {
  ctx.t(match mode {
    ComposerMode::Steer(_) => "runs.conversation.composer.sendAfterTurn",
    ComposerMode::Queue(_) | ComposerMode::Closed => "conversation.composer.send",
    ComposerMode::Answer { .. } => "runs.conversation.composer.sendAnswer",
    ComposerMode::PlanChanges { .. } => "runs.plan.requestChanges",
    ComposerMode::FollowUp => "runs.conversation.composer.sendFollowUp",
    ComposerMode::Continue => "runs.conversation.composer.continue",
    ComposerMode::NotStarted => "runs.conversation.composer.sendWhenStarts",
  })
}

/// One recipient as the select offers it in `mode`.
pub(super) fn recipient_label(
  ctx: &Ctx,
  conversation: &Conversation,
  mode: &ComposerMode,
  choice: &RecipientChoice,
) -> Arc<str> {
  let Some(agent) = choice.agent.as_deref() else {
    return match (mode, conversation.first_agent.as_deref()) {
      (ComposerMode::NotStarted, Some(first)) => ctx.t_args("runs.conversation.to.firstStep", [("agent", first)]),
      _ => ctx.t("conversation.to.orchestrator"),
    };
  };
  let working = matches!(choice.recipient, Recipient::Agent(profile)
    if conversation.working.iter().any(|working| working.profile == profile));
  let key = match mode {
    _ if working => "runs.conversation.to.working",
    ComposerMode::FollowUp => "runs.conversation.to.resumes",
    ComposerMode::Continue => "runs.conversation.to.continues",
    _ => "conversation.to.agent",
  };
  ctx.t_args(key, [("agent", agent)])
}

/// What sending does now (`pDLcP`).
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
    ComposerMode::Queue(queued) => queue_hint(ctx, queued, conversation.state),
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

/// Above the composer (`k0NkK` in `ZxYGh`): *Answering Builder's question from 12:09*, *Changes to
/// plan v1 · sent to Planner*; *Waiting for your decision* with the way to it, on the task's Summary.
pub(super) fn context_line(ctx: &mut Ctx, mode: &ComposerMode, task: TaskId) -> Option<Element> {
  let (glyph, text) = match mode {
    ComposerMode::Queue(Queued::Decision) => return Some(decision_line(ctx, task)),
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
    _ => return None,
  };
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
      .child(
        Text::new(&text)
          .id("conversation-composer-context")
          .variant(TypographyStyle::Caption),
      )
      .into(),
  )
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
