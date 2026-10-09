//! One run in *Runs of this task* (`Run row` `Db8k4`): its status icon, *Run #N* (tagged *this run* when
//! it is the run the page shows) with where it stands, what happened, its worktree line, when it last
//! changed, and at the end *Remove worktree* while it has worktrees and no live work. The removal's
//! confirmation opens in the row; while git removes, the button says so and the task's other rows
//! wait; git's refusal stays under the row until the person asks again.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{integration::Leftovers, run::RunNumber};
use orchester_workspace::{
  messages::Message,
  tasks::{
    history::RunRecord,
    integration_jobs::{JobState, RunJobState},
  },
};

use super::line::{Line, day_and_clock, line};
use crate::{
  project::ShownProject,
  screens::task::{PageState, RemovalPlace},
  theme::{ExtraSpacing, ExtraTypography, IconSize, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    status::Status,
  },
};

#[derive(Clone, PartialEq)]
pub struct RowProps {
  pub project: ShownProject,
  pub key: Arc<str>,
  pub record: RunRecord,
  /// What the run left as last read, for its line; `None` for a run with live work.
  pub left: Option<Result<Leftovers, Message>>,
  /// What the run left as read now, while it has worktrees and no live work: it can be removed.
  pub removable: Option<Leftovers>,
  pub this_run: bool,
  pub job: RunJobState,
  /// Another run of the task is being removed or merged.
  pub others_busy: bool,
  /// Another run of the task at work.
  pub working: Option<RunNumber>,
  /// Its removal's confirmation is open, in the row or under the header.
  pub confirming: bool,
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

pub struct RunRow;

impl Component for RunRow {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let record = &props.record;
    let status = record.phase.map_or(Status::Unreadable, Status::of_run);
    let failed = match &props.job.remove {
      JobState::Failed(error) => Some(error.clone()),
      _ => None,
    };
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Xs)
      .child(head(ctx, &props, status))
      .child(
        Text::new(&message_text(ctx, &record.detail))
          .id(format!("run-row-{}-what", record.id))
          .variant(TypographyStyle::Description)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(line(ctx, record, props.left.as_ref()).map(|line| worktree_line(&line)))
      .with_children(failed.as_ref().filter(|_| !props.confirming).map(|error| {
        Text::new(&message_text(ctx, error))
          .id(format!("run-remove-error-{}", record.id))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Danger)
          .width(Dimension::Pct(100.0))
      }));
    let when = record
      .at
      .map_or_else(|| ctx.t("summary.runs.noTime"), |at| day_and_clock(ctx, at));
    let main = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(icon(status.icon(), IconSize::Md, status.color()))
      .child(text)
      .child(Text::new(&when).variant(ExtraTypography::Note.role()).nowrap())
      .with_children(self.action(ctx, &props));
    Column::new()
      .id(format!("run-row-{}", record.id))
      .describe("run", record.number.map_or(0, RunNumber::get).to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
      .child(main)
  }
}

impl RunRow {
  /// *Remove worktree*, while the run can be removed and its confirmation is closed.
  fn action(&self, ctx: &mut Ctx, props: &RowProps) -> Option<Element> {
    props.removable.as_ref()?;
    let number = props.record.number.map_or(0, RunNumber::get);
    let button = ActionButton::new(
      format!("run-remove-worktree-{number}"),
      ButtonKind::Secondary,
      ctx.t("task.removeWorktree"),
    )
    .icon(Icon::FolderX);
    if props.job.removing() {
      return Some(button.loading().into());
    }
    if props.confirming {
      return None;
    }
    if props.job.busy() || props.others_busy {
      return Some(button.disabled().into());
    }
    let (state, run) = (PageState::of(ctx), props.record.id);
    let jobs = props.project.feed.jobs.of(run);
    Some(
      button
        .on_click(move || {
          jobs.clear_failed_removal();
          state.open_removal(run, RemovalPlace::Row);
        })
        .into(),
    )
  }
}

/// "Run #128", the *this run* tag and where the run stands.
fn head(ctx: &mut Ctx, props: &RowProps, status: Status) -> Element {
  let record = &props.record;
  let title = record.number.map_or_else(
    || ctx.t("summary.runs.runUnnumbered"),
    |number| ctx.t_args("summary.runs.run", [("run", number.get().to_string())]),
  );
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&title).variant(TaskText::RunTitle.role()).nowrap())
    .with_children(props.this_run.then(|| {
      Text::new(&ctx.t("summary.runs.thisRun"))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .child(
      Text::new(&ctx.t(status.key()))
        .variant(TypographyStyle::Description)
        .color(status.color())
        .nowrap(),
    )
    .into()
}

fn worktree_line(line: &Line) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(line.glyph, IconSize::Sm, line.color.clone()))
    .child(
      Text::new(&line.text)
        .variant(ExtraTypography::Note.role())
        .color(line.color.clone())
        .flex(1.0),
    )
    .into()
}
