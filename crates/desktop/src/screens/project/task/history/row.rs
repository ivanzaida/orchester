//! One run in *Runs of this task* (`V8nI6` in 63.01; stacked at 960, `abxEG` in 63.02): *Run #N*
//! (tagged *this run* when it is the run the page shows), its status label (`DeizF`), what happened,
//! its worktree line and when, and at the right *Remove worktree* (`u1XBP`, 32 high) while it has
//! worktrees and no live work. Its states (63.06 `VBR27`): *Removing…* while git removes them, the
//! other rows' buttons waiting; git's refusal under the worktree line with the button offered again;
//! removed.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  integration::Leftovers,
  run::{RunNumber, TaskPhase},
};

use super::{
  super::{
    super::{ProjectNav, task_rows::row_status_label},
    worktree::{Place, RemoveWorktree, RemoveWorktreeProps},
  },
  line::{Line, day_and_clock, line},
};
use crate::{
  messages::Message,
  tasks::{history::RunRecord, integration::unread_entry, integration_jobs::JobState},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    field::wrapping_field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::status_label,
  },
  workspace::OpenProject,
};

/// Cells of a row with columns (`pE8A6`, `aqYtY`, `x6pfC`, `H1REtJ` in 63.01).
const RUN_WIDTH: f32 = 76.0;
const STATUS_WIDTH: f32 = 150.0;
const WHEN_WIDTH: f32 = 96.0;
const ACTION_WIDTH: f32 = 150.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RunRowProps {
  pub(super) open: OpenProject,
  pub(super) task: TaskId,
  pub(super) key: Arc<str>,
  pub(super) record: RunRecord,
  /// What the run left, once read; `None` for a run with live work.
  pub(super) left: Option<Result<Leftovers, Message>>,
  pub(super) this_run: bool,
  /// Whether its button shows the confirmation (the topbar does for the latest run).
  pub(super) dialog: bool,
  pub(super) others: Arc<[RunId]>,
  pub(super) working: Option<RunNumber>,
  /// Whether the window is wide enough for columns.
  pub(super) wide: bool,
}

impl lurq::app::component::DevtoolsInspectable for RunRowProps {}

pub(super) struct RunRow {
  nav: ProjectNav,
}

impl Component for RunRow {
  type Props = RunRowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunRowProps>().clone();
    // Git's refusal stays under the row until the person tries again (63.06 B).
    let error = match self.nav.integration(props.record.id).remove().get() {
      JobState::Failed(error) => Some(message_text(ctx, &error)),
      _ => None,
    };
    let parts = Parts {
      run: run_cell(ctx, &props),
      status: status(ctx, props.record.phase),
      what: what(ctx, &props, error.as_deref()),
      when: when(ctx, &props.record),
      action: action(ctx, &props),
    };
    let row = if props.wide { columns(parts) } else { stacked(parts) };
    Column::new()
      .id(format!("run-row-{}", props.record.id))
      .describe("run", props.record.number.map_or(0, RunNumber::get).to_string())
      .width(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Group.role())
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(row)
  }
}

struct Parts {
  run: Vec<Element>,
  status: Element,
  what: Vec<Element>,
  when: Element,
  action: Option<Element>,
}

/// Run, status, what happened with its lines, when and the action side by side (63.01).
fn columns(parts: Parts) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .width(Dimension::Px(RUN_WIDTH))
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Start)
        .with_children(parts.run),
    )
    .child(Row::new().width(Dimension::Px(STATUS_WIDTH)).child(parts.status))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .with_children(parts.what),
    )
    .child(Row::new().width(Dimension::Px(WHEN_WIDTH)).child(parts.when))
    .child(
      Row::new()
        .min_width(Dimension::Px(ACTION_WIDTH))
        .justify(Justify::End)
        .with_children(parts.action),
    )
    .into()
}

/// Run, status and the action on one line, what happened, its lines and when under it (63.02).
fn stacked(parts: Parts) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .with_children(parts.run)
        .child(parts.status)
        .child(Spacer::new().flex(1.0))
        .with_children(parts.action),
    )
    .with_children(parts.what)
    .child(parts.when)
    .into()
}

/// Where the run stands (`DeizF`); a run whose every step passed reads *Ready for review*, as its
/// page's heading says, rather than the task list's *Completed* (63.06).
fn status(ctx: &Ctx, phase: Option<TaskPhase>) -> Element {
  match phase {
    Some(TaskPhase::Succeeded) => status_label(Icon::GitPullRequest, &ctx.t("runs.ready.status")),
    phase => row_status_label(ctx, phase),
  }
}

/// "Run #134" and, for the run the page shows, the *this run* tag (`fLmx4`).
fn run_cell(ctx: &Ctx, props: &RunRowProps) -> Vec<Element> {
  let number = props.record.number.map_or_else(
    || ctx.t("runs.history.runUnnumbered"),
    |number| ctx.t_args("runs.history.run", [("run", number.get().to_string().as_str())]),
  );
  let tag = props.this_run.then(|| {
    Row::new()
      .padding_vertical(ExtraSpacing::Hair.role())
      .padding_horizontal(ExtraSpacing::Tight.role())
      .rounded(ExtraRadius::TabControl.role())
      .background(PaletteColor::AccentMuted)
      .child(
        Text::new(&ctx.t("runs.history.thisRun"))
          .variant(ExtraTypography::Hint.role())
          .nowrap(),
      )
      .into()
  });
  std::iter::once(
    Text::new(&number)
      .variant(ExtraTypography::FormLabel.role())
      .nowrap()
      .into(),
  )
  .chain(tag)
  .collect()
}

/// What happened, the worktree line, why a folder git forgot was not read to its end, and git's
/// refusal, if any.
fn what(ctx: &Ctx, props: &RunRowProps, error: Option<&str>) -> Vec<Element> {
  let happened = Text::new(&message_text(ctx, &props.record.detail))
    .variant(ExtraTypography::CellLabel.role())
    .color(PaletteColor::TextPrimary)
    .width(Dimension::Pct(100.0));
  let worktree = line(ctx, &props.record, props.left.as_ref()).map(|line| worktree_line(&line));
  let unread: Vec<Element> = match &props.left {
    Some(Ok(left)) => left
      .worktrees
      .iter()
      .filter_map(unread_entry)
      .map(|problem| wrapping_field_error(&message_text(ctx, &problem)))
      .collect(),
    _ => Vec::new(),
  };
  std::iter::once(happened.into())
    .chain(worktree)
    .chain(unread)
    .chain(error.map(|error| {
      Column::new()
        .id(format!("run-remove-error-{}", props.record.id))
        .width(Dimension::Pct(100.0))
        .child(wrapping_field_error(error))
        .into()
    }))
    .collect()
}

fn worktree_line(line: &Line) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Start)
    .child(icon(line.glyph, IconSize::Small, line.color.clone()))
    .child(
      Text::new(&line.text)
        .variant(ExtraTypography::CellLabel.role())
        .color(line.color.clone())
        .flex(1.0),
    )
    .into()
}

/// When the run last changed; a dash while it cannot be read.
fn when(ctx: &Ctx, record: &RunRecord) -> Element {
  let at = record
    .at
    .and_then(|at| day_and_clock(ctx, at))
    .unwrap_or_else(|| ctx.t("runs.history.noTime"));
  Text::new(&at)
    .variant(TypographyStyle::Mono)
    .color(PaletteColor::TextMuted)
    .nowrap()
    .into()
}

/// *Remove worktree*, while the run has worktrees and no live work.
fn action(ctx: &mut Ctx, props: &RunRowProps) -> Option<Element> {
  let run = props.record.without_live_work()?;
  let leftovers = props.left.clone()?.ok().filter(|left| !left.worktrees.is_empty())?;
  let removal = RemoveWorktreeProps {
    open: props.open.clone(),
    task: props.task,
    key: props.key.clone(),
    run: run.clone(),
    leftovers,
    place: Place::Row { dialog: props.dialog },
    others: props.others.clone(),
    working: props.working,
  };
  Some(ctx.mount_keyed::<RemoveWorktree>(&format!("run-remove-{}", props.record.id), removal))
}
