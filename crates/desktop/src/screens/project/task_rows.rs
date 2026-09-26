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
  tasks::list::TaskRow,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
    status_label::status_label,
  },
};

/// Task table cell widths and row heights (`B5FaIC`, `clVjO`).
const SELECT_WIDTH: f32 = 18.0;
const KEY_WIDTH: f32 = 72.0;
const PIPELINE_WIDTH: f32 = 136.0;
/// Wider than `B5FaIC`'s 112, so the longest status, "Stopped at limit" (`BHqwT`), fits.
const STATUS_WIDTH: f32 = 128.0;
const READINESS_WIDTH: f32 = 146.0;
const OPEN_WIDTH: f32 = 14.0;
const HEADING_HEIGHT: f32 = 28.0;
const TABLE_ROW_HEIGHT: f32 = 48.0;
/// Overview row: row height, state tile and the width of the time column (`s0gWDM`, `aqRNL`,
/// `XY1PR`).
const LIST_ROW_HEIGHT: f32 = 78.0;
const STATE_TILE_SIZE: f32 = 32.0;
const TIME_WIDTH: f32 = 60.0;

fn status_copy(phase: TaskPhase) -> (Icon, &'static str) {
  match phase {
    TaskPhase::Draft => (Icon::Pencil, "tasks.status.draft"),
    TaskPhase::Queued => (Icon::Clock3, "tasks.status.queued"),
    TaskPhase::Running => (Icon::Loader, "tasks.status.running"),
    TaskPhase::Waiting => (Icon::Clock3, "tasks.status.waiting"),
    TaskPhase::Paused => (Icon::Pause, "tasks.status.paused"),
    TaskPhase::StoppedAtLimit => (Icon::OctagonPause, "tasks.status.stoppedAtLimit"),
    TaskPhase::Interrupted => (Icon::Pause, "tasks.status.interrupted"),
    TaskPhase::Succeeded => (Icon::CircleCheck, "tasks.status.succeeded"),
    TaskPhase::Failed => (Icon::AlertCircle, "tasks.status.failed"),
    TaskPhase::Cancelled => (Icon::Square, "tasks.status.cancelled"),
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
  let (nav, task) = (nav.clone(), row.id);
  pressable(surface(), move || nav.show_task(task))
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
    .child(cell(PIPELINE_WIDTH, column_label(&ctx.t("tasks.columns.pipeline"))))
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
    let (glyph, status) = status_copy(row.phase);
    // Selecting tasks for a run is not available yet (`R7idX`).
    let select = icon(Icon::Square, IconSize::Option, PaletteColor::TextMuted);
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
        Row::new().flex(1.0).align_items(Alignment::Center).child(
          Text::new(&row.title)
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .flex(1.0),
        ),
      )
      .child(cell(
        PIPELINE_WIDTH,
        column_label(&message_text(ctx, &row.pipeline.message())).text_overflow(TextOverflow::Elipsis),
      ))
      .child(cell(STATUS_WIDTH, status_label(glyph, &ctx.t(status))))
      .child(cell(READINESS_WIDTH, column_label(&message_text(ctx, &row.detail))))
      .child(open_chevron());
    opens(&self.nav, &row, content)
  }
}

/// A task in the overview's task list (`zemhy`): state tile, title with key and readiness,
/// status, age.
pub fn list_row(ctx: &Ctx, nav: &ProjectNav, row: &TaskRow, now: Timestamp) -> Element {
  let (glyph, status) = status_copy(row.phase);
  let tile = Row::new()
    .size(Dimension::Px(STATE_TILE_SIZE), Dimension::Px(STATE_TILE_SIZE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Option, PaletteColor::TextPrimary));
  let metadata = Row::new()
    .spacing(SpacingSize::Xs)
    .child(Text::new(&row.key).variant(TypographyStyle::Caption).nowrap())
    .child(Text::new(&ctx.t("common.separator")).variant(TypographyStyle::Caption))
    .child(Text::new(&message_text(ctx, &row.detail)).variant(TypographyStyle::Caption));
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
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(metadata),
    )
    .child(status_label(glyph, &ctx.t(status)))
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
