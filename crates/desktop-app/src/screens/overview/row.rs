//! One task in the Overview's list (`Overview row` `XirGV`): its title with its first two tags, its key,
//! workflow status and where it stands under them, its run's status with the step it is at, and its
//! priority's icon; 10 over and under it and 14 at its sides, on a `border` line. A press opens the task's
//! Summary; rows are pointer-only.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::list::TaskRow;

use super::open_task;
use crate::{
  routes::TaskTab,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    chip::TagChip,
    icon::icon,
    message::message_text,
    pressable::pressable,
    status::{Status, StatusChip, priority_look, workflow_key},
    tooltip::Truncated,
  },
};

/// The tags a row names (`Tag 1`, `Tag 2` in `XirGV`).
const SHOWN_TAGS: usize = 2;
/// The status column (`Status` `wqb5q`).
const STATUS_WIDTH: f32 = 200.0;

pub(super) fn row(ctx: &mut Ctx, project: ProjectId, row: &TaskRow) -> Button {
  let key = &row.key;
  let open = open_task(ctx, project, key, TaskTab::Summary);
  let (priority, ..) = priority_look(row.organization.priority());
  pressable(open)
    .id(format!("overview-row-{key}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("task", key.clone())
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(title_line(ctx, row))
        .child(sub_line(ctx, row)),
    )
    .child(Row::new().width(Dimension::Px(STATUS_WIDTH)).child(status(ctx, row)))
    .child(icon(priority, IconSize::Md, PaletteColor::TextMuted).id(format!("overview-priority-{key}")))
}

/// The title, cut to its room, and the first tags.
fn title_line(ctx: &mut Ctx, row: &TaskRow) -> Row {
  let key = &row.key;
  let tags = row
    .tags
    .iter()
    .take(SHOWN_TAGS)
    .enumerate()
    .map(|(index, name)| TagChip::new(format!("overview-tag-{key}-{index}"), name.clone()));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Truncated::new(
        format!("overview-title-{key}"),
        row.title.clone(),
        ExtraTypography::Strong.role(),
      )
      .render(ctx),
    )
    .with_children(tags)
}

/// *KON-12 · In progress · Running*: the key, the workflow status and where the task stands.
fn sub_line(ctx: &mut Ctx, row: &TaskRow) -> Row {
  let separator = ctx.t("overview.separator");
  let dot = || Text::new(&separator).variant(TypographyStyle::Caption);
  let detail = message_text(ctx, &row.detail);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(&row.key).variant(ExtraTypography::MonoNote.role()).nowrap())
    .child(dot())
    .child(
      Text::new(&ctx.t(workflow_key(row.organization.status())))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .child(dot())
    .child(
      Truncated::new(format!("overview-detail-{}", row.key), detail, TypographyStyle::Caption)
        .color(PaletteColor::TextSecondary)
        .render(ctx),
    )
}

/// The run's status with the step it is at or stopped at (`Running · 03 Implement`).
fn status(ctx: &mut Ctx, row: &TaskRow) -> lurq::node::Element {
  let status = row.phase.map_or(Status::Unreadable, Status::of_phase);
  let mut chip = StatusChip::new(format!("overview-status-{}", row.key), status);
  if let Some(step) = &row.step {
    let number = format!("{:02}", step.number);
    chip = chip.detail(ctx.t_args(
      "overview.tasks.runStep",
      [("number", number.as_str()), ("step", &*step.name)],
    ));
  }
  chip.render(ctx)
}
