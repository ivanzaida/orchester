//! The closing panel's runs, one group per project (`Group · In progress · Kontur` `sWhP4`): its title,
//! then a row per run with an agent at work (`Closing run row` `y6v6zN`): the task's key and title over
//! its step and agent, where the run stands, and provider · model or, while waiting, how the run is
//! saving or where it paused (`Row states` in `mKfVy`). Every value is runtime data.

use std::{sync::Arc, time::SystemTime};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::{
  display,
  runner::{ActiveWork, SaveState},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, LifecycleSpacing},
  ui::icon::{Icon, icon},
};

/// The width of a row's status and of its last cell (`Status` `gowVm`, `Last` `i0xS0`).
const CELL_WIDTH: f32 = 220.0;

/// The order in which a row's cells give way when the window is narrow (`Narrow window (960)` in
/// `mKfVy`): the last cell first, then the title; the step, the status and the actions never do.
mod give_way {
  pub const LAST: i32 = 0;
  pub const TITLE: i32 = 1;
}

/// A run of the panel and where it stands.
pub struct Shown<'a> {
  pub work: &'a ActiveWork,
  pub stands: Stands,
}

/// Where a run stands on the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stands {
  /// The person decides: its agent works.
  Running,
  /// Its agent finishes up to its next checkpoint; the runner last reported an event of it then.
  Saving { last_event: Option<SystemTime> },
  /// Paused, at this checkpoint (a full revision) or without a new one.
  Saved { checkpoint: Option<Arc<str>> },
}

impl Stands {
  /// While waiting: what the runner reported of the run (`None`: nothing yet, it still saves).
  pub fn waiting(save: Option<SaveState>, last_event: Option<SystemTime>) -> Self {
    match save {
      Some(SaveState::Saved { checkpoint }) => Self::Saved { checkpoint },
      Some(SaveState::Saving) | None => Self::Saving { last_event },
    }
  }

  pub const fn is_saved(&self) -> bool {
    matches!(self, Self::Saved { .. })
  }
}

/// The group of project `project`'s runs, titled `title`.
pub fn group(ctx: &Ctx, project: &str, title: &str, runs: &[Shown<'_>]) -> Element {
  let last = runs.len().saturating_sub(1);
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(
      runs
        .iter()
        .enumerate()
        .map(|(index, shown)| row(ctx, shown, index == last)),
    );
  Column::new()
    .id(format!("closing-group-{project}"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(title)
        .id(format!("closing-group-title-{project}"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary),
    )
    .child(rows)
    .into()
}

fn row(ctx: &Ctx, shown: &Shown<'_>, last: bool) -> Element {
  let work = shown.work;
  let run = work.run();
  let step = ctx.t_args(
    "lifecycle.close.row.stepAgent",
    [("step", work.step()), ("agent", work.agent())],
  );
  let task = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(work.task_key())
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextMuted)
            .nowrap()
            .flex_shrink(0.0),
        )
        .child(
          Text::new(work.task_title())
            .id(format!("closing-title-{run}"))
            .variant(ExtraTypography::Strong.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .flex_shrink(1.0)
            .shrink_order(give_way::TITLE),
        ),
    )
    .child(
      Text::new(&step)
        .id(format!("closing-step-{run}"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    );
  let line = Row::new()
    .id(format!("closing-row-{run}"))
    .describe("stands", stands_name(&shown.stands))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, LifecycleSpacing::RowEdge.role()))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(task)
    .child(status(ctx, &shown.stands, run))
    .child(last_cell(ctx, work, &shown.stands));
  if last {
    line.into()
  } else {
    line
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

const fn stands_name(stands: &Stands) -> &'static str {
  match stands {
    Stands::Running => "running",
    Stands::Saving { .. } => "saving",
    Stands::Saved { .. } => "saved",
  }
}

/// *Running*, *Saving at next checkpoint…* or *Saved*, with its icon (`Status` `gowVm`).
fn status(ctx: &Ctx, stands: &Stands, run: impl std::fmt::Display) -> Row {
  let (glyph, color, words) = match stands {
    Stands::Running => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.row.running",
    ),
    Stands::Saving { .. } => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.row.saving",
    ),
    Stands::Saved { .. } => (Icon::CircleCheck, PaletteColor::Success, "lifecycle.close.row.saved"),
  };
  Row::new()
    .id(format!("closing-status-{run}"))
    .width(Dimension::Px(CELL_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&ctx.t(words))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
}

/// Provider · model; while waiting, how long ago its agent last did something, or where it paused.
fn last_cell(ctx: &Ctx, work: &ActiveWork, stands: &Stands) -> Row {
  let words = match stands {
    Stands::Running => ctx.t_args(
      "lifecycle.close.row.providerModel",
      [
        ("provider", &*ctx.t(provider_key(work.provider()))),
        ("model", work.model()),
      ],
    ),
    Stands::Saving { last_event } => last_event_words(ctx, *last_event),
    Stands::Saved {
      checkpoint: Some(revision),
    } => ctx.t_args(
      "lifecycle.close.row.pausedAt",
      [("checkpoint", display::short_revision(revision))],
    ),
    Stands::Saved { checkpoint: None } => ctx.t("lifecycle.close.row.paused"),
  };
  Row::new()
    .width(Dimension::Px(CELL_WIDTH))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .shrink_order(give_way::LAST)
    .justify(Justify::End)
    .child(
      Text::new(&words)
        .id(format!("closing-last-{}", work.run()))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}

/// *Step still running · last event 3 s ago*, measured from the runner's last report of the run.
fn last_event_words(ctx: &Ctx, last_event: Option<SystemTime>) -> Arc<str> {
  let Some(elapsed) = last_event.and_then(|at| at.elapsed().ok()) else {
    return ctx.t("lifecycle.close.row.stillRunning");
  };
  let seconds = elapsed.as_secs();
  if seconds < 60 {
    ctx.t_args(
      "lifecycle.close.row.lastEvent.seconds",
      [("seconds", seconds.to_string())],
    )
  } else {
    ctx.t_args(
      "lifecycle.close.row.lastEvent.minutes",
      [("minutes", (seconds / 60).to_string())],
    )
  }
}

pub(super) const fn provider_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "lifecycle.close.provider.codex",
    AgentProvider::ClaudeCodeCli => "lifecycle.close.provider.claude",
    AgentProvider::AnthropicApi => "lifecycle.close.provider.anthropicApi",
    AgentProvider::OpenAiApi => "lifecycle.close.provider.openaiApi",
  }
}
