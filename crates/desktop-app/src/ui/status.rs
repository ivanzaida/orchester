//! Status labels: every status is an icon and a word (handover §5.1). A status (`Status` `VWRbG`) is a
//! 13 icon and its word in 12/500, 5 apart, both in the status's tone; the compact one a table row's
//! run line uses is a 12 icon and 11.5/500 (`Run line` `AgVPe`). A priority (`Priority cell` `OfuTC`) is
//! a 13 icon and its word in 12, `text_secondary`; a workflow status reads like a priority.
//!
//! The vocabulary and its icons come from the boards: Draft `circle-dashed`, Queued `clock`, Running and
//! Working `loader-circle` (turning), Needs input `hand`, Stopped at limit `gauge`, Interrupted and
//! Interrupted · can't continue `circle-pause`, Completed, Complete and Ready for review `circle-check`, Merged
//! `git-merge`, Not merged `triangle-alert` (`kxMAN`), Failed `circle-x`, Unreadable `circle-help`, Waiting `clock`,
//! Not started `circle-dashed`, Blocked `ban`, Paused `circle-pause` in `warning` (Usage `Q1gYMm`); Backlog
//! `circle-dashed`, Todo `circle` (`AiCwO`), In progress `circle-dot-dashed`, In review `eye`, Done
//! `circle-check`; High, Medium and Low `signal-*`, No priority `minus`. The boards draw no Canceled,
//! Skipped or Urgent: they take `circle-slash`, `skip-forward` and `circle-alert` (`warning`). A status
//! with a detail is cut to its row's room, its whole words in a tooltip.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  run::TaskPhase,
  task::organization::{Priority, WorkflowStatus},
};

use super::{
  icon::{Icon, icon},
  tooltip::Truncated,
};
use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// What a run or one of its steps is doing, and the Blocked flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
  Draft,
  Queued,
  Running,
  NeedsInput,
  Paused,
  StoppedAtLimit,
  Interrupted,
  Completed,
  /// A run whose every step passed, as its own page and its row in *Runs of this task* name it
  /// (`F2LtFn`); task lists say Completed.
  ReadyForReview,
  /// A run ready for review whose merge failed and left its base branch as it was.
  NotMerged,
  Merged,
  Failed,
  Canceled,
  Unreadable,
  Waiting,
  Working,
  Complete,
  Skipped,
  NotStarted,
  CantContinue,
  Blocked,
}

/// The tones a status is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
  Muted,
  Secondary,
  Warning,
  Success,
  Danger,
}

impl Tone {
  const fn color(self) -> PaletteColor {
    match self {
      Self::Muted => PaletteColor::TextMuted,
      Self::Secondary => PaletteColor::TextSecondary,
      Self::Warning => PaletteColor::Warning,
      Self::Success => PaletteColor::Success,
      Self::Danger => PaletteColor::Danger,
    }
  }
}

impl Status {
  pub const ALL: [Self; 21] = [
    Self::Draft,
    Self::Queued,
    Self::Running,
    Self::NeedsInput,
    Self::Paused,
    Self::StoppedAtLimit,
    Self::Interrupted,
    Self::Completed,
    Self::ReadyForReview,
    Self::NotMerged,
    Self::Merged,
    Self::Failed,
    Self::Canceled,
    Self::Unreadable,
    Self::Waiting,
    Self::Working,
    Self::Complete,
    Self::Skipped,
    Self::NotStarted,
    Self::CantContinue,
    Self::Blocked,
  ];

  /// The run label of a task in `phase`; a run that waits for a person, for an answer or after its
  /// retries, needs input.
  pub const fn of_phase(phase: TaskPhase) -> Self {
    match phase {
      TaskPhase::Draft => Self::Draft,
      TaskPhase::Queued => Self::Queued,
      TaskPhase::Running => Self::Running,
      TaskPhase::Waiting | TaskPhase::Stuck => Self::NeedsInput,
      TaskPhase::Paused => Self::Paused,
      TaskPhase::StoppedAtLimit => Self::StoppedAtLimit,
      TaskPhase::Interrupted => Self::Interrupted,
      TaskPhase::Succeeded => Self::Completed,
      TaskPhase::Merged => Self::Merged,
      TaskPhase::Failed => Self::Failed,
      TaskPhase::Cancelled => Self::Canceled,
    }
  }

  /// The label of a run in `phase` on its task's page: as [`Self::of_phase`], but a run whose every step
  /// passed waits for review (`Ready for review` in `F2LtFn`).
  pub const fn of_run(phase: TaskPhase) -> Self {
    match phase {
      TaskPhase::Succeeded => Self::ReadyForReview,
      phase => Self::of_phase(phase),
    }
  }

  pub const fn icon(self) -> Icon {
    match self {
      Self::Draft | Self::NotStarted => Icon::CircleDashed,
      Self::Queued | Self::Waiting => Icon::Clock,
      Self::Running | Self::Working => Icon::LoaderCircle,
      Self::NeedsInput => Icon::Hand,
      Self::Paused | Self::Interrupted | Self::CantContinue => Icon::CirclePause,
      Self::StoppedAtLimit => Icon::Gauge,
      Self::Completed | Self::Complete | Self::ReadyForReview => Icon::CircleCheck,
      Self::Merged => Icon::GitMerge,
      Self::NotMerged => Icon::TriangleAlert,
      Self::Failed => Icon::CircleX,
      Self::Canceled => Icon::CircleSlash,
      // The boards name `file-warning`, which Pencil has no glyph for and draws as `circle-help`; the
      // design shows `circle-help` (`Run icon` in `XEQuo` rows of `C2E5Po`).
      Self::Unreadable => Icon::CircleHelp,
      Self::Skipped => Icon::SkipForward,
      Self::Blocked => Icon::Ban,
    }
  }

  const fn tone(self) -> Tone {
    match self {
      Self::Draft | Self::Queued | Self::Canceled | Self::Waiting | Self::Skipped | Self::NotStarted => Tone::Muted,
      Self::Running | Self::Merged | Self::Working | Self::Blocked => Tone::Secondary,
      Self::NeedsInput
      | Self::Paused
      | Self::StoppedAtLimit
      | Self::Interrupted
      | Self::CantContinue
      | Self::NotMerged => Tone::Warning,
      Self::Completed | Self::Complete | Self::ReadyForReview => Tone::Success,
      Self::Failed | Self::Unreadable => Tone::Danger,
    }
  }

  /// The color of the icon and the word.
  pub const fn color(self) -> PaletteColor {
    self.tone().color()
  }

  /// The i18n key of the word.
  pub const fn key(self) -> &'static str {
    match self {
      Self::Draft => "ui.status.draft",
      Self::Queued => "ui.status.queued",
      Self::Running => "ui.status.running",
      Self::NeedsInput => "ui.status.needsInput",
      Self::Paused => "ui.status.paused",
      Self::StoppedAtLimit => "ui.status.stoppedAtLimit",
      Self::Interrupted => "ui.status.interrupted",
      Self::Completed => "ui.status.completed",
      Self::ReadyForReview => "ui.status.readyForReview",
      Self::NotMerged => "ui.status.notMerged",
      Self::Merged => "ui.status.merged",
      Self::Failed => "ui.status.failed",
      Self::Canceled => "ui.status.canceled",
      Self::Unreadable => "ui.status.unreadable",
      Self::Waiting => "ui.status.waiting",
      Self::Working => "ui.status.working",
      Self::Complete => "ui.status.complete",
      Self::Skipped => "ui.status.skipped",
      Self::NotStarted => "ui.status.notStarted",
      Self::CantContinue => "ui.status.cantContinue",
      Self::Blocked => "ui.status.blocked",
    }
  }
}

/// How large a label is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StatusSize {
  /// 13 icon, 12/500 word.
  #[default]
  Regular,
  /// 12 icon, 11.5/500 word: a table row's run line.
  Compact,
}

/// A status's icon and word, with an optional detail after it (`Running · 03 Implement`).
pub struct StatusChip {
  id: Arc<str>,
  status: Status,
  detail: Option<Arc<str>>,
  size: StatusSize,
}

impl StatusChip {
  pub fn new(id: impl Into<Arc<str>>, status: Status) -> Self {
    Self {
      id: id.into(),
      status,
      detail: None,
      size: StatusSize::Regular,
    }
  }

  /// What the status is about, after its word: a step, a reason.
  pub fn detail(mut self, detail: impl Into<Arc<str>>) -> Self {
    self.detail = Some(detail.into());
    self
  }

  pub fn compact(mut self) -> Self {
    self.size = StatusSize::Compact;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let word = ctx.t(self.status.key());
    let text = match &self.detail {
      Some(detail) => ctx.t_args("ui.status.withDetail", [("status", word), ("detail", detail.clone())]),
      None => word,
    };
    let (icon_size, typography) = match self.size {
      StatusSize::Regular => (IconSize::Base, TypographyStyle::Label),
      StatusSize::Compact => (IconSize::Sm, ExtraTypography::NoteStrong.role()),
    };
    let color = self.status.color();
    Row::new()
      .id(self.id.to_string())
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .describe("status", self.status.key())
      .child(icon(self.status.icon(), icon_size, color.clone()).id(format!("{}-icon", self.id)))
      .child(
        Truncated::new(format!("{}-label", self.id), text, typography)
          .color(color)
          .render(ctx),
      )
      .into()
  }
}

/// A task's priority: its icon and word in `text_secondary`; Urgent in `warning`, No priority in
/// `text_muted`.
pub fn priority_chip(ctx: &Ctx, id: &str, priority: Priority) -> Row {
  let (glyph, key, color) = priority_look(priority);
  labelled(
    id,
    (glyph, IconSize::Base),
    &ctx.t(key),
    TypographyStyle::Description,
    color,
  )
}

/// A priority's icon, the i18n key of its word and its color.
pub const fn priority_look(priority: Priority) -> (Icon, &'static str, PaletteColor) {
  match priority {
    Priority::Urgent => (Icon::CircleAlert, "ui.priority.urgent", PaletteColor::Warning),
    Priority::High => (Icon::SignalHigh, "ui.priority.high", PaletteColor::TextSecondary),
    Priority::Medium => (Icon::SignalMedium, "ui.priority.medium", PaletteColor::TextSecondary),
    Priority::Low => (Icon::SignalLow, "ui.priority.low", PaletteColor::TextSecondary),
    Priority::None => (Icon::Minus, "ui.priority.none", PaletteColor::TextMuted),
  }
}

/// A task's workflow status: its icon and word in `text_secondary`; Done's icon in `success`.
pub fn workflow_chip(ctx: &Ctx, id: &str, status: WorkflowStatus) -> Row {
  let (glyph, tone) = match status {
    WorkflowStatus::Backlog => (Icon::CircleDashed, PaletteColor::TextMuted),
    WorkflowStatus::Todo => (Icon::Circle, PaletteColor::TextSecondary),
    WorkflowStatus::InProgress => (Icon::CircleDotDashed, PaletteColor::TextSecondary),
    WorkflowStatus::InReview => (Icon::Eye, PaletteColor::TextSecondary),
    WorkflowStatus::Done => (Icon::CircleCheck, PaletteColor::Success),
    WorkflowStatus::Canceled => (Icon::CircleSlash, PaletteColor::TextMuted),
  };
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone).id(format!("{id}-icon")))
    .child(
      Text::new(&ctx.t(workflow_key(status)))
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
}

/// The i18n key of a workflow status's word.
pub const fn workflow_key(status: WorkflowStatus) -> &'static str {
  match status {
    WorkflowStatus::Backlog => "ui.workflow.backlog",
    WorkflowStatus::Todo => "ui.workflow.todo",
    WorkflowStatus::InProgress => "ui.workflow.inProgress",
    WorkflowStatus::InReview => "ui.workflow.inReview",
    WorkflowStatus::Done => "ui.workflow.done",
    WorkflowStatus::Canceled => "ui.workflow.canceled",
  }
}

fn labelled(
  id: &str,
  (glyph, size): (Icon, IconSize),
  text: &str,
  typography: TypographyStyle,
  color: PaletteColor,
) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, size, color.clone()).id(format!("{id}-icon")))
    .child(
      Text::new(text)
        .id(format!("{id}-label"))
        .variant(typography)
        .color(color)
        .nowrap(),
    )
}
