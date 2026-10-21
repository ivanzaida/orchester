//! One task in the Overview's list (`Overview row` `XirGV`): its title with its first two tags, its key,
//! workflow status and where it stands under them, its run's status with the step it is at, and its
//! priority's icon; 10 over and under it and 14 at its sides, on a `border` line. A press opens the task on its
//! Details; rows are pointer-only. After the tasks, a line that is no task's (`Row · project memory`
//! `S9Zgf`): what waits, what it is about and its action.

use std::sync::Arc;

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
use orchester_workspace::tasks::{
  board::ProjectRow,
  list::{RunStage, TaskRow},
};

use super::open_task;
use crate::{
  navigation::{Navigation, waiting::Destination},
  routes::TaskTab,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    status::{Status, StatusChip, priority_look, workflow_key},
    tooltip::Truncated,
    unavailable::reason_line,
  },
};

/// The tags a row names (`Tag 1`, `Tag 2` in `XirGV`).
const SHOWN_TAGS: usize = 2;
/// The status column (`Status` `wqb5q`).
const STATUS_WIDTH: f32 = 200.0;
/// The project's line's id, apart from the tasks' `overview-row-<key>`.
const PROJECT_ROW: &str = "overview-memory-row";

/// The project's memory records waiting for approval, in the brain's `warning` (`S9Zgf`). *Review records*
/// leads where the *Needs you* card's does (`navigation::waiting`): the project's Memory page.
pub(super) fn project_row(ctx: &mut Ctx, project: ProjectId, row: &ProjectRow) -> Row {
  let destination = Destination::of(project, &row.item).map_err(|reason| message_text(ctx, &reason));
  let reason = destination.as_ref().err().cloned();
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&message_text(ctx, &row.item.title))
        .id(format!("{PROJECT_ROW}-title"))
        .variant(ExtraTypography::Strong.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&message_text(ctx, &row.detail))
        .id(format!("{PROJECT_ROW}-detail"))
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(
      reason
        .as_ref()
        .map(|reason| reason_line(&format!("{PROJECT_ROW}-reason"), reason)),
    );
  let review = ActionButton::new(
    format!("{PROJECT_ROW}-action"),
    ButtonKind::Ghost,
    message_text(ctx, &row.item.kind.action()),
  )
  .icon(Icon::ArrowUpRight);
  let review = match destination {
    Ok(destination) => {
      let navigation = Navigation::of(ctx);
      review.opens(Arc::new(move |at| destination.open(&navigation, at)))
    }
    Err(reason) => review.unavailable(reason),
  };
  Row::new()
    .id(PROJECT_ROW)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::Brain, IconSize::Md, PaletteColor::Warning))
    .child(text)
    .child(Row::new().flex_shrink(0.0).child(review))
}

pub(super) fn row(ctx: &mut Ctx, project: ProjectId, row: &TaskRow) -> Button {
  let key = &row.key;
  let open = open_task(ctx, project, key, TaskTab::default());
  let (priority, ..) = priority_look(row.organization.priority());
  opening(open)
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

/// The run's status with where it stands (`Running · 03 Implement`, `Ready to merge · Deliver`).
fn status(ctx: &mut Ctx, row: &TaskRow) -> lurq::node::Element {
  let status = match row.phase {
    None => Status::Unreadable,
    // Nothing to merge: the run completed, and nothing of it waits to be merged.
    Some(phase) if row.nothing_to_merge => Status::of_phase(phase),
    Some(phase) => Status::of_board(phase),
  };
  let mut chip = StatusChip::new(format!("overview-status-{}", row.key), status);
  match RunStage::of(row.phase, row.step.as_ref(), row.nothing_to_merge) {
    Some(RunStage::Step(step)) => {
      let number = format!("{:02}", step.number);
      chip = chip.detail(ctx.t_args(
        "overview.tasks.runStep",
        [("number", number.as_str()), ("step", &*step.name)],
      ));
    }
    Some(RunStage::Deliver) => chip = chip.detail(ctx.t("overview.tasks.deliver")),
    None => {}
  }
  chip.render(ctx)
}
