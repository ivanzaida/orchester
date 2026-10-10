//! The closing panel's runs, one group per project (`Group · In progress · Kontur` `sWhP4`): its title,
//! then a row per run with an agent at work (`Closing run row` `y6v6zN`): the task's key and title over
//! its step and agent, where the run stands, and provider · model or, while waiting, how the run is
//! saving or where it paused (`Row states` in `mKfVy`). Git's merges and worktree removals under way get a
//! group of their own, in the same rows. Every value is runtime data.

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
use orchester_workspace::{close::RunStands, display, runner::ActiveWork, tasks::integration_jobs::BusyJob};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, LifecycleSpacing, LifecycleText},
  ui::icon::{Icon, icon},
};

/// The width of a row's status and of its last cell (`Status` `gowVm`; `Last` as the panels widen it).
const STATUS_WIDTH: f32 = 220.0;
const LAST_WIDTH: f32 = 250.0;

/// The order in which a row's cells give way when the window is narrow (`Narrow window (960)` in
/// `mKfVy`): the last cell first, then the title; the step, the status and the actions never do.
mod give_way {
  pub const LAST: i32 = 0;
  pub const TITLE: i32 = 1;
}

/// A run of the panel, where it stands, and when its runner last reported an event of it.
pub struct Shown<'a> {
  pub work: &'a ActiveWork,
  pub stands: &'a RunStands,
  pub last_event: Option<SystemTime>,
}

/// One row's words: its id, key and title, the line under them, its status and the last cell.
struct Line<'a> {
  id: String,
  key: &'a str,
  title: &'a str,
  detail: Arc<str>,
  status: (Icon, PaletteColor, Arc<str>),
  last: Arc<str>,
  stands: &'static str,
}

/// The group of project `project`'s runs, titled `title`.
pub fn group(ctx: &Ctx, project: &str, title: &str, runs: &[Shown<'_>]) -> Element {
  let lines: Vec<Line<'_>> = runs.iter().map(|shown| run_line(ctx, shown)).collect();
  framed(&format!("closing-group-{project}"), title, lines)
}

/// Git's operations in project `project` (`Git work · Kontur`).
pub fn jobs_group(ctx: &Ctx, project: &str, jobs: &[&BusyJob]) -> Element {
  let title = ctx.t_args("lifecycle.close.group.git", [("project", project)]);
  let lines: Vec<Line<'_>> = jobs.iter().map(|job| job_line(ctx, job)).collect();
  framed(&format!("closing-git-{project}"), &title, lines)
}

fn framed(id: &str, title: &str, lines: Vec<Line<'_>>) -> Element {
  let last = lines.len().saturating_sub(1);
  let rows = Column::new()
    .width(Dimension::Pct(100.0))
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(
      lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| row(line, index == last)),
    );
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary),
    )
    .child(rows)
    .into()
}

fn run_line<'a>(ctx: &Ctx, shown: &Shown<'a>) -> Line<'a> {
  let work = shown.work;
  let (glyph, color, words, stands) = match shown.stands {
    RunStands::Running => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.row.running",
      "running",
    ),
    RunStands::Saving => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "lifecycle.close.row.saving",
      "saving",
    ),
    RunStands::Saved { .. } => (
      Icon::CircleCheck,
      PaletteColor::Success,
      "lifecycle.close.row.saved",
      "saved",
    ),
  };
  Line {
    id: work.run().to_string(),
    key: work.task_key(),
    title: work.task_title(),
    detail: ctx.t_args(
      "lifecycle.close.row.stepAgent",
      [("step", work.step()), ("agent", work.agent())],
    ),
    status: (glyph, color, ctx.t(words)),
    last: last_words(ctx, shown),
    stands,
  }
}

fn job_line<'a>(ctx: &Ctx, job: &'a BusyJob) -> Line<'a> {
  let (detail, status) = if job.merge {
    ("lifecycle.close.row.merge", "lifecycle.close.row.merging")
  } else {
    ("lifecycle.close.row.removal", "lifecycle.close.row.removing")
  };
  Line {
    id: format!("{}-{}", job.task_key, job.run_number),
    key: &job.task_key,
    title: "",
    detail: ctx.t_args(detail, [("run", job.run_number.to_string())]),
    status: (Icon::LoaderCircle, PaletteColor::TextSecondary, ctx.t(status)),
    last: ctx.t("lifecycle.close.row.git"),
    stands: "git",
  }
}

fn row(line: Line<'_>, last: bool) -> Element {
  let id = &line.id;
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
          Text::new(line.key)
            .variant(LifecycleText::RowKey.role())
            .nowrap()
            .flex_shrink(0.0),
        )
        .child(
          Text::new(line.title)
            .id(format!("closing-title-{id}"))
            .variant(ExtraTypography::Strong.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .flex_shrink(1.0)
            .shrink_order(give_way::TITLE),
        ),
    )
    .child(
      Text::new(&line.detail)
        .id(format!("closing-step-{id}"))
        .variant(LifecycleText::RowText.role())
        .nowrap(),
    );
  let (glyph, color, words) = line.status;
  let status = Row::new()
    .id(format!("closing-status-{id}"))
    .width(Dimension::Px(STATUS_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&words)
        .variant(LifecycleText::RowText.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    );
  let last_cell = Row::new()
    .width(Dimension::Px(LAST_WIDTH))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .shrink_order(give_way::LAST)
    .justify(Justify::End)
    .child(
      Text::new(&line.last)
        .id(format!("closing-last-{id}"))
        .variant(LifecycleText::RowText.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let row = Row::new()
    .id(format!("closing-row-{id}"))
    .describe("stands", line.stands)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, LifecycleSpacing::RowEdge.role()))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(task)
    .child(status)
    .child(last_cell);
  if last {
    row.into()
  } else {
    row
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// Provider · model; while waiting, how long ago its agent last did something, or where it paused.
fn last_words(ctx: &Ctx, shown: &Shown<'_>) -> Arc<str> {
  let work = shown.work;
  match shown.stands {
    RunStands::Running => ctx.t_args(
      "lifecycle.close.row.providerModel",
      [
        ("provider", &*ctx.t(provider_key(work.provider()))),
        ("model", work.model()),
      ],
    ),
    RunStands::Saving => last_event_words(ctx, shown.last_event),
    RunStands::Saved {
      checkpoint: Some(revision),
    } => ctx.t_args(
      "lifecycle.close.row.pausedAt",
      [("checkpoint", display::short_revision(revision))],
    ),
    RunStands::Saved { checkpoint: None } => ctx.t("lifecycle.close.row.paused"),
  }
}

/// *Step still running · last event 3 s ago*, measured from the runner's last report of the run; without
/// one yet, *Step still running*.
fn last_event_words(ctx: &Ctx, last_event: Option<SystemTime>) -> Arc<str> {
  let Some(at) = last_event else {
    return ctx.t("lifecycle.close.row.stillRunning");
  };
  // A report from after now (the clock was set back since) is as recent as can be.
  let seconds = at.elapsed().map_or(0, |elapsed| elapsed.as_secs());
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
