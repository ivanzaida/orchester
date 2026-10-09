//! The tab's toolbar (`Toolbar` `qLARF`): 46 high, a `border` line under it, 32 in from the sides, with the run
//! state chip at its end (`Run state · Following live` `AQbG3`): a 24 pill in a `border` stroke, its 12 icon and
//! its words in 11.5 `text_secondary`, 6 apart, 9 from its sides. While an agent works it says whether the chat
//! follows it (*Following live*, `radio` in `success`; *Not following*, `pause`); otherwise where the run
//! stands (*Queued*, `clock`, `D1h8Z`), in the words the run's state has. *Show*, the search and Details are
//! not part of this build (D4).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::run::TaskPhase;
use orchester_workspace::tasks::{
  conversation::{Conversation, PanelState},
  detail::TaskDetail,
};

use crate::{
  theme::{ConversationSpacing, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The toolbar's height (`qLARF`) and the chip's (`AQbG3`).
const TOOLBAR_HEIGHT: f32 = 46.0;
const CHIP_HEIGHT: f32 = 24.0;

/// Where the run stands, as the chip says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RunState {
  /// An agent works, and the chat follows it or not.
  Live {
    following: bool,
  },
  /// The task has no run yet.
  NoRun,
  /// Its latest run cannot be read: where it stands is unknown.
  Unreadable,
  Queued,
  /// A question, a plan or a tool request waits for the person.
  Waiting,
  Deciding,
  Paused,
  Stopped,
  Interrupted,
  Ready,
  Merged,
  Failed,
  Cancelled,
}

impl RunState {
  /// The state of `conversation`, the task `detail`'s; `asking`: an agent waits in its turn for the answer
  /// to its question.
  pub(super) fn of(conversation: &Conversation, detail: &TaskDetail, asking: bool) -> Self {
    if detail.latest_unreadable.is_some() {
      return Self::Unreadable;
    }
    match conversation.state {
      PanelState::NotStarted if detail.run.is_some() || detail.phase() == TaskPhase::Queued => Self::Queued,
      PanelState::NotStarted => Self::NoRun,
      PanelState::Live if asking => Self::Waiting,
      PanelState::Live => Self::Live { following: true },
      PanelState::Waiting => Self::Waiting,
      PanelState::Deciding => Self::Deciding,
      PanelState::Paused => Self::Paused,
      PanelState::Stopped => Self::Stopped,
      PanelState::Interrupted => Self::Interrupted,
      PanelState::Ready | PanelState::WorktreeRemoved => Self::Ready,
      PanelState::Merged => Self::Merged,
      PanelState::Failed | PanelState::FailedWorktreeRemoved | PanelState::FailedNoAgent => Self::Failed,
      PanelState::Ended => Self::Cancelled,
    }
  }

  /// While an agent works: whether the chat follows it.
  pub(super) const fn following(self, following: bool) -> Self {
    match self {
      Self::Live { .. } => Self::Live { following },
      other => other,
    }
  }

  /// The chip's icon, its color and its words; `None` when there is no run to say anything of, or it cannot
  /// be read.
  fn look(self) -> Option<(Icon, PaletteColor, &'static str)> {
    let muted = PaletteColor::TextMuted;
    Some(match self {
      Self::Live { following: true } => (Icon::Radio, PaletteColor::Success, "conversation.state.following"),
      Self::Live { following: false } => (Icon::Pause, muted, "conversation.state.notFollowing"),
      Self::NoRun | Self::Unreadable => return None,
      Self::Queued => (Icon::Clock, muted, "conversation.state.queued"),
      Self::Waiting => (Icon::Hand, PaletteColor::Warning, "conversation.state.waiting"),
      Self::Deciding => (Icon::Hand, PaletteColor::Warning, "conversation.state.deciding"),
      Self::Paused => (Icon::Pause, muted, "conversation.state.paused"),
      Self::Stopped => (Icon::OctagonPause, muted, "conversation.state.stopped"),
      Self::Interrupted => (Icon::CirclePause, muted, "conversation.state.interrupted"),
      Self::Ready => (Icon::GitPullRequest, muted, "conversation.state.ready"),
      Self::Merged => (Icon::GitMerge, muted, "conversation.state.merged"),
      Self::Failed => (Icon::CircleX, PaletteColor::Danger, "conversation.state.failed"),
      Self::Cancelled => (Icon::Square, muted, "conversation.state.cancelled"),
    })
  }

  /// The name `describe` reports the state by, for tests and QA.
  const fn name(self) -> &'static str {
    match self {
      Self::Live { following: true } => "following",
      Self::Live { following: false } => "not-following",
      Self::NoRun => "no-run",
      Self::Unreadable => "unreadable",
      Self::Queued => "queued",
      Self::Waiting => "waiting",
      Self::Deciding => "deciding",
      Self::Paused => "paused",
      Self::Stopped => "stopped",
      Self::Interrupted => "interrupted",
      Self::Ready => "ready",
      Self::Merged => "merged",
      Self::Failed => "failed",
      Self::Cancelled => "cancelled",
    }
  }
}

pub(super) fn toolbar(ctx: &Ctx, state: RunState) -> Element {
  let chip = state.look().map(|(glyph, color, key)| {
    Row::new()
      .id("conversation-run-state")
      .describe("state", state.name())
      .height(Dimension::Px(CHIP_HEIGHT))
      .padding_horizontal(ConversationSpacing::StateChip.role())
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Full.role())
      .child(icon(glyph, IconSize::Sm, color))
      .child(
        Text::new(&ctx.t(key))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
  });
  Row::new()
    .id("conversation-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .flex_shrink(0.0)
    .padding_horizontal(SpacingSize::Section)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Spacer::new().flex(1.0))
    .with_children(chip)
    .into()
}
