//! What the composer says in each state of the run: its primary button, each recipient (*To: Builder
//! · working*, *To: Builder · resumes its session*, *To: Explorer · first step*) and the line under
//! it that says what sending does now (`docs/design/orchester-workspace.md`, "Composer").

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::conversation::Recipient;

use crate::tasks::conversation::{
  Conversation, PanelState, RecipientChoice,
  composer::{ComposerMode, Queued},
};

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
