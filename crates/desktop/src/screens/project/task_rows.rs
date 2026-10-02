//! Task rows: the task table row of the task list (`clVjO`) with its heading (`B5FaIC`), and
//! the task list row of the overview (`zemhy`). A row opens the task's page; rows are outside a
//! form, so they are pointer-only.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{run::TaskPhase, time::Timestamp};

use super::ProjectNav;
use crate::{
  display,
  navigation::route::Page,
  tasks::{
    list::TaskRow,
    organization::{priority_label, workflow_label},
    tag_chips::row_tag_chips,
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::surface,
    status_label::{danger_status_label, status_label},
  },
};

/// Task table cell widths and row heights (`B5FaIC`, `clVjO`). Status fits the longest status,
/// "Stopped at limit" (`BHqwT`, cell `VizsW`); dependency / readiness fits its longest text on one
/// line (`S2kP9L`).
const SELECT_WIDTH: f32 = 18.0;
const KEY_WIDTH: f32 = 72.0;
const PRIORITY_WIDTH: f32 = 96.0;
// ORC-39 qVMJh: workflow and run state occupy separate lines.
const STATUS_WIDTH: f32 = 212.0;
const READINESS_WIDTH: f32 = 184.0;
const OPEN_WIDTH: f32 = 14.0;
const HEADING_HEIGHT: f32 = 28.0;
const TABLE_ROW_HEIGHT: f32 = 76.0;
/// Overview row: row height, state tile and the width of the time column (`s0gWDM`, `aqRNL`,
/// `XY1PR`).
// DtpHJ full-width overview task specimen.
const LIST_ROW_HEIGHT: f32 = 136.0;
const STATE_TILE_SIZE: f32 = 32.0;
const TIME_WIDTH: f32 = 60.0;

/// The task status label (`DeizF`), as its variants in `P3HONM` draw it; what each means for the
/// task is the row's last column (`r2al55`).
pub(super) fn status_copy(phase: TaskPhase) -> (Icon, &'static str) {
  match phase {
    TaskPhase::Draft => (Icon::Pencil, "tasks.status.draft"),
    TaskPhase::Queued => (Icon::Clock3, "tasks.status.queued"),
    TaskPhase::Running => (Icon::Loader, "tasks.status.running"),
    TaskPhase::Waiting => (Icon::MessageCircle, "tasks.status.waiting"),
    TaskPhase::Paused => (Icon::Pause, "tasks.status.paused"),
    TaskPhase::StoppedAtLimit => (Icon::OctagonPause, "tasks.status.stoppedAtLimit"),
    TaskPhase::Interrupted => (Icon::CirclePause, "tasks.status.interrupted"),
    TaskPhase::Succeeded => (Icon::CircleCheck, "tasks.status.succeeded"),
    TaskPhase::Merged => (Icon::GitMerge, "tasks.status.merged"),
    TaskPhase::Failed => (Icon::CircleX, "tasks.status.failed"),
    TaskPhase::Cancelled => (Icon::Square, "tasks.status.cancelled"),
  }
}

/// The icon of a task row's state: where the task stands, or circle-alert when its latest run
/// cannot be read (`phase` is `None`).
fn row_glyph(phase: Option<TaskPhase>) -> Icon {
  phase.map_or(Icon::AlertCircle, |phase| status_copy(phase).0)
}

/// The status label of a task row or run: where it stands, or *Unreadable* in the danger role when
/// its run cannot be read (`phase` is `None`, `uEyyL` in `mGbYs`).
pub(super) fn row_status_label(ctx: &Ctx, phase: Option<TaskPhase>) -> Element {
  match phase {
    Some(phase) => {
      let (glyph, status) = status_copy(phase);
      status_label(glyph, &ctx.t(status))
    }
    None => danger_status_label(Icon::AlertCircle, &ctx.t("tasks.status.unreadable")),
  }
}

fn cell(width: f32, content: impl Into<Element>) -> Element {
  Row::new()
    .width(Dimension::Px(width))
    .align_items(Alignment::Center)
    .child(content)
    .into()
}

/// The open chevron (`B1lur`).
fn open_chevron() -> Element {
  cell(
    OPEN_WIDTH,
    icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted),
  )
}

/// `row` as a surface that opens the task's page.
fn opens(nav: &ProjectNav, row: &TaskRow, content: Row) -> Element {
  nav
    .link(Page::task(row.id))
    .on(surface())
    .id(format!("task-row-{}", row.key))
    .width(Dimension::Pct(100.0))
    .describe("task", row.key.clone())
    .child(content)
    .into()
}

fn column_label(text: &str) -> Text {
  Text::new(text).variant(ExtraTypography::Footnote.role()).nowrap()
}

/// Column labels of the task table (`B5FaIC`).
pub fn table_heading(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(Row::new().width(Dimension::Px(SELECT_WIDTH)))
    .child(cell(KEY_WIDTH, column_label(&ctx.t("tasks.columns.key"))))
    .child(Row::new().flex(1.0).child(column_label(&ctx.t("tasks.columns.task"))))
    .child(cell(PRIORITY_WIDTH, column_label(&ctx.t("tasks.priority.label"))))
    .child(cell(STATUS_WIDTH, column_label(&ctx.t("tasks.columns.status"))))
    .child(cell(READINESS_WIDTH, column_label(&ctx.t("tasks.columns.readiness"))))
    .child(Row::new().width(Dimension::Px(OPEN_WIDTH)))
    .into()
}

/// One row of the task table (`clVjO`), mounted by the virtualized task list.
pub struct QueueRow {
  nav: ProjectNav,
}

impl Component for QueueRow {
  type Props = TaskRow;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let row = ctx.props::<TaskRow>().clone();
    // Selecting tasks for a run is not available yet: unselected in `$muted` (`R7idX` in
    // `r2al55`).
    let select = selection_checkbox(ctx, row.id);
    let content = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TABLE_ROW_HEIGHT))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .child(cell(SELECT_WIDTH, select))
      .child(cell(
        KEY_WIDTH,
        Text::new(&row.key).variant(TypographyStyle::Mono).nowrap(),
      ))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&row.title)
              .id(format!("task-row-title-{}", row.key))
              .variant(TypographyStyle::FieldLabel)
              .color(PaletteColor::TextPrimary)
              .nowrap()
              .text_overflow(TextOverflow::Elipsis)
              .width(Dimension::Pct(100.0)),
          )
          .child(row_tag_chips(ctx, row.id, &row.tags)),
      )
      .child(cell(PRIORITY_WIDTH, priority_label(ctx, row.organization.priority())))
      .child(cell(STATUS_WIDTH, workflow_run(ctx, &row)))
      .child(cell(
        READINESS_WIDTH,
        Text::new(&message_text(ctx, &row.detail))
          .variant(ExtraTypography::CellLabel.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .width(Dimension::Pct(100.0)),
      ))
      .child(open_chevron());
    opens(&self.nav, &row, content)
  }
}

/// A task in the overview's task list (`zemhy`): state tile, title with key and readiness,
/// status, age.
pub fn list_row(ctx: &mut Ctx, nav: &ProjectNav, row: &TaskRow, now: Timestamp) -> Element {
  let tile = Row::new()
    .size(Dimension::Px(STATE_TILE_SIZE), Dimension::Px(STATE_TILE_SIZE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(row_glyph(row.phase), IconSize::Option, PaletteColor::TextPrimary));
  let metadata = Row::new()
    .wrap()
    .spacing(SpacingSize::Xs)
    .child(Text::new(&row.key).variant(TypographyStyle::Caption).nowrap())
    .child(Text::new(&ctx.t("common.separator")).variant(TypographyStyle::Caption))
    .child(
      Text::new(&message_text(ctx, &row.detail))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .max_width(Dimension::Pct(65.0)),
    )
    .child(priority_label(ctx, row.organization.priority()));
  let content = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LIST_ROW_HEIGHT))
    .spacing(ExtraSpacing::Group.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Indicator.role())
        .child(
          Text::new(&row.title)
            .variant(TypographyStyle::Label)
            .width(Dimension::Pct(100.0))
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(metadata),
    )
    .child(overview_workflow(ctx, row))
    .child(row_tag_chips(ctx, row.id, &row.tags))
    .child(
      Row::new().width(Dimension::Px(TIME_WIDTH)).justify(Justify::End).child(
        Text::new(&message_text(ctx, &display::elapsed(now, row.created_at)))
          .variant(TypographyStyle::Mono)
          .nowrap(),
      ),
    )
    .child(open_chevron());
  opens(nav, row, content)
}

fn workflow_run(ctx: &Ctx, row: &TaskRow) -> Element {
  let run_key = system_label_key(row.phase).unwrap_or("tasks.workflow.noActiveRun");
  Column::new()
    .spacing(SpacingSize::Xs)
    .child(workflow_label(ctx, row.organization.status()))
    .child(Text::new(&ctx.t(run_key)).variant(TypographyStyle::Caption))
    .with_children(is_blocked(row).then(|| {
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleMinus, IconSize::Small, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("tasks.blocked.label")).variant(TypographyStyle::Caption))
    }))
    .into()
}

fn selection_checkbox(ctx: &mut Ctx, id: orchester_core::id::TaskId) -> Element {
  let Some(super::task_selection::TaskSelection(selected)) = ctx.use_context::<super::task_selection::TaskSelection>()
  else {
    return icon(Icon::Square, IconSize::Option, PaletteColor::TextSecondary).into();
  };
  let checked = selected.get().0.contains(&id);
  surface()
    .id(format!("select-task-{id}"))
    .cursor(lurq::node::CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |event: lurq::app::events::MouseEvent| {
      event.stop_propagation();
      selected.update(|value| {
        if !value.0.remove(&id) {
          value.0.insert(id);
        }
      });
    })
    .child(icon(
      if checked { Icon::SquareCheck } else { Icon::Square },
      IconSize::Option,
      PaletteColor::TextSecondary,
    ))
    .into()
}

fn overview_workflow(ctx: &Ctx, row: &TaskRow) -> Element {
  let (icon, key) = crate::tasks::organization::workflow_copy(row.organization.status());
  let mut label = ctx.t(key);
  for suffix in [
    system_label_key(row.phase),
    is_blocked(row).then_some("tasks.blocked.label"),
  ]
  .into_iter()
  .flatten()
  {
    label = ctx.t_args(
      "tasks.workflow.combined",
      [("status", label.as_ref()), ("run", ctx.t(suffix).as_ref())],
    );
  }
  status_label(icon, &label)
}

fn system_label_key(phase: Option<TaskPhase>) -> Option<&'static str> {
  match phase {
    Some(TaskPhase::Draft | TaskPhase::Queued | TaskPhase::Merged | TaskPhase::Succeeded) => None,
    Some(phase) => Some(status_copy(phase).1),
    None => Some("tasks.status.unreadable"),
  }
}

fn is_blocked(row: &TaskRow) -> bool {
  row.organization.blocked_reason().is_some()
    || matches!(row.detail.key, "tasks.blocked.unfinished" | "tasks.readiness.blocked")
}
