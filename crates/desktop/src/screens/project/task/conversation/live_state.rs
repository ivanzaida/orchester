//! Where the run stands, at the right of the timeline's toolbar (`DeizF`): while an agent works,
//! whether the timeline follows it (*Following live*, `h8oM7`; *Not following*, `YtSJY`); otherwise
//! the run's state — *Waiting for you* for a question (`O3ezCC`) or a plan or tool request (`nfcrN`),
//! *Queued* (`Z9B5ld`), *Ready for review* (`RqYsy`), *Failed* (`Uo3IM`) — and, where no frame draws
//! one, the task status's own words.

use lurq::{app::ctx::Ctx, node::Element};

use crate::{
  tasks::conversation::PanelState,
  ui::{icon::Icon, status_label::compact_status_label},
};

/// The label for a run in `state`; `asks`: an agent's question waits for the person.
pub(super) fn live_state(ctx: &Ctx, state: PanelState, asks: bool, following: bool) -> Element {
  let (glyph, key) = match state {
    PanelState::Live if following => (Icon::Radio, "runs.activity.following"),
    PanelState::Live => (Icon::CirclePause, "runs.activity.notFollowing"),
    PanelState::Waiting if asks => (Icon::MessageCircle, "runs.conversation.state.waiting"),
    PanelState::Waiting => (Icon::Hand, "runs.conversation.state.waiting"),
    PanelState::NotStarted => (Icon::Clock3, "runs.conversation.state.queued"),
    PanelState::Deciding => (Icon::Hand, "runs.activity.waitingDecision"),
    PanelState::Paused => (Icon::Pause, "runs.conversation.state.paused"),
    PanelState::Stopped => (Icon::OctagonPause, "runs.conversation.state.stopped"),
    PanelState::Interrupted => (Icon::CirclePause, "runs.conversation.state.interrupted"),
    PanelState::Ready | PanelState::WorktreeRemoved => (Icon::GitPullRequest, "runs.conversation.state.ready"),
    PanelState::Merged => (Icon::GitMerge, "runs.conversation.state.merged"),
    PanelState::Failed | PanelState::FailedWorktreeRemoved | PanelState::FailedNoAgent => {
      (Icon::CircleX, "runs.conversation.state.failed")
    }
    PanelState::Ended => (Icon::Square, "runs.conversation.state.cancelled"),
  };
  compact_status_label(glyph, &ctx.t(key)).id("conversation-live-state")
}
