//! A run of the merge queue (`Queue row` `kcUhy`): its state (a 14 glyph and its word, the line under it) in a 168
//! wide column, the run (task key and title, the meta line, and under it the phases of its delivery, the files a
//! dirty checkout holds, or the detail its state has) and its actions: *Move up*, *Move to top* and *Remove* on a
//! waiting run, *Open task* on every other run, *Review & merge* on one waiting for approval, *Retry* on a blocked one.
//! 16 apart, 12 over and under and 16 at the sides, over a `border` line.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  merge_queue::{Blocked, QueueState},
};
use orchester_workspace::merge_queue::{Move, QueueRow};

use super::{actions::QueueActions, phases, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The width of a row's state column (`State` in `kcUhy`).
const STATE_WIDTH: f32 = 168.0;

/// What every row of a queue shares: its project, the person's checkout, whether waiting runs are held behind a
/// blocked one, and what its buttons do.
#[derive(Clone)]
pub(super) struct Rows {
  pub project: ProjectId,
  pub checkout: Arc<str>,
  pub held: bool,
  pub actions: QueueActions,
}

/// `row` of the queue.
pub(super) fn row(ctx: &mut Ctx, rows: &Rows, row: &QueueRow) -> Element {
  let id = format!("merge-queue-row-{}", row.run_number);
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Lg))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .describe("state", state_name(&row.entry.state))
    .child(state(ctx, &id, row))
    .child(run(ctx, &id, rows, row))
    .child(buttons(ctx, &id, rows, row))
    .into()
}

fn state(ctx: &Ctx, id: &str, row: &QueueRow) -> Column {
  let look = words::state(ctx, row);
  Column::new()
    .id(format!("{id}-state"))
    .width(Dimension::Px(STATE_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(look.icon, IconSize::Md, look.icon_color))
        .child(
          Text::new(&look.label)
            .id(format!("{id}-state-label"))
            .variant(DeliveryText::State.role())
            .color(look.label_color),
        ),
    )
    .child(
      Text::new(&words::sub(ctx, row))
        .id(format!("{id}-sub"))
        .variant(ExtraTypography::Note.role()),
    )
}

fn run(ctx: &mut Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Column {
  let head = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&row.task_key.to_string()).variant(DeliveryText::Count.role()))
    .child(
      Text::new(&row.task_title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::Strong.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Column::new()
    .id(format!("{id}-run"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(head)
    .child(
      Text::new(&words::meta(ctx, row))
        .id(format!("{id}-meta"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary),
    )
    .with_children(phases::phases(
      ctx,
      id,
      (
        &row.phases(),
        phases::Named {
          base: words::base(row),
          integrator: row.integrator.as_deref(),
        },
      ),
    ))
    .with_children(uncommitted(ctx, id, rows, row))
    .with_children(detail(ctx, id, rows, row))
}

/// The files a dirty checkout holds, under a blocked run's phases.
fn uncommitted(ctx: &Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Option<Element> {
  let QueueState::Blocked(Blocked::Uncommitted { files }) = &row.entry.state else {
    return None;
  };
  let label = ctx.t_args("delivery.queue.blocked.uncommittedIn", [("path", &*rows.checkout)]);
  Some(files_line(&format!("{id}-uncommitted"), &label, files).into())
}

/// `label` and the files in mono after it, 8 apart.
pub(crate) fn files_line(id: &str, label: &str, files: &[String]) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .wrap()
    .child(Text::new(label).variant(ExtraTypography::Note.role()))
    .with_children(
      files
        .iter()
        .map(|file| Text::new(file).variant(DeliveryText::File.role())),
    )
}

fn detail(ctx: &Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Option<Element> {
  let detail = match &row.entry.state {
    QueueState::Queued if rows.held => Some(ctx.t("delivery.queue.blocked.held")),
    _ => words::detail(ctx, row),
  }?;
  Some(
    Text::new(&detail)
      .id(format!("{id}-detail"))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
      .into(),
  )
}

fn buttons(ctx: &mut Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Row {
  let navigation = Navigation::of(ctx);
  let task_page = navigation.to(Route::Project(
    rows.project,
    ProjectPage::Task(row.task_key.clone(), TaskView::Summary),
  ));
  let open_task = ActionButton::new(
    format!("{id}-open-task"),
    ButtonKind::Ghost,
    ctx.t("delivery.queue.action.openTask"),
  )
  .icon(Icon::ExternalLink)
  .opens(task_page.clone());
  let buttons: Vec<ActionButton> = match &row.entry.state {
    QueueState::Queued => waiting_buttons(ctx, id, rows, row),
    QueueState::WaitingForApproval => vec![
      open_task,
      ActionButton::new(
        format!("{id}-review"),
        ButtonKind::Primary,
        ctx.t("delivery.queue.action.reviewAndMerge"),
      )
      .icon(Icon::GitMerge)
      .opens(task_page),
    ],
    QueueState::Blocked(_) => vec![
      open_task,
      rows.actions.retries(
        ActionButton::new(
          format!("{id}-retry"),
          ButtonKind::Secondary,
          ctx.t("delivery.queue.action.retry"),
        )
        .icon(Icon::RotateCcw),
      ),
    ],
    _ => vec![open_task],
  };
  Row::new()
    .id(format!("{id}-actions"))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(buttons)
}

/// *Move up*, *Move to top* and *Remove*; the first waiting run moves nowhere.
fn waiting_buttons(ctx: &Ctx, id: &str, rows: &Rows, row: &QueueRow) -> Vec<ActionButton> {
  let run = row.entry.run;
  let first = row.place == Some(1);
  let moving = |button: ActionButton, how| {
    if first {
      button.disabled()
    } else {
      rows.actions.moves(button, run, how)
    }
  };
  vec![
    moving(
      ActionButton::new(
        format!("{id}-move-up"),
        ButtonKind::Ghost,
        ctx.t("delivery.queue.action.moveUp"),
      )
      .icon(Icon::ArrowUp),
      Move::Up,
    ),
    moving(
      ActionButton::new(
        format!("{id}-move-to-top"),
        ButtonKind::Ghost,
        ctx.t("delivery.queue.action.moveToTop"),
      )
      .icon(Icon::ArrowUpToLine),
      Move::ToTop,
    ),
    rows.actions.removes(
      ActionButton::new(
        format!("{id}-remove"),
        ButtonKind::Ghost,
        ctx.t("delivery.queue.action.remove"),
      )
      .icon(Icon::X),
      run,
    ),
  ]
}

const fn state_name(state: &QueueState) -> &'static str {
  match state {
    QueueState::Queued => "queued",
    QueueState::Syncing => "syncing",
    QueueState::Rechecking { .. } => "rechecking",
    QueueState::Integrating { .. } => "integrating",
    QueueState::WaitingForApproval => "waiting_for_approval",
    QueueState::Blocked(_) => "blocked",
    QueueState::Left(_) => "left",
    QueueState::Merged(_) => "merged",
    QueueState::Removed => "removed",
  }
}
