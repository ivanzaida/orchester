//! The review's table (`Join header` `izogn`, `Join row` `cA02b` in `AiCwO`): a checkbox, the local key, the title with
//! the server's similar task as a hint or why the task can't be unticked, where the task and its latest run stand, and
//! what it becomes: its new key, or *Not shared*. A task with a run that has not ended stays ticked, its box faded.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::TaskPhase;
use orchester_workspace::sharing::project::ReviewTask;

use super::plan::{Planned, Unticked};
use crate::{
  screens::settings::orchester::PersonOnly,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    status::{Status, workflow_chip},
    table::{ColumnWidth, table_header},
    toggle::{Toggle, ToggleKind},
  },
};

/// The columns (`izogn`): the box 16, the local key 72, the task the room left, workflow and run 200, what it becomes
/// 120.
const TICK: ColumnWidth = ColumnWidth::Fixed(16.0);
const KEY: ColumnWidth = ColumnWidth::Fixed(72.0);
const TASK: ColumnWidth = ColumnWidth::Fill;
const WORKFLOW: ColumnWidth = ColumnWidth::Fixed(200.0);
const BECOMES: ColumnWidth = ColumnWidth::Fixed(120.0);

/// The table of `rows`; a press on a box that can be unticked changes `unticked`.
pub(super) fn table(
  ctx: &mut Ctx,
  rows: &[Planned<'_>],
  (key, unticked, person): (&str, &Signal<Unticked>, &PersonOnly),
) -> Element {
  let headings = [
    (TICK, None),
    (KEY, Some(ctx.t("sharing.join.columns.key"))),
    (TASK, Some(ctx.t("sharing.join.columns.task"))),
    (WORKFLOW, Some(ctx.t("sharing.join.columns.workflow"))),
    (BECOMES, Some(ctx.t("sharing.join.columns.becomes"))),
  ];
  let columns: Vec<(ColumnWidth, Option<&str>)> = headings
    .iter()
    .map(|(width, heading)| (*width, heading.as_deref()))
    .collect();
  let mut table = Column::new()
    .id("sharing-join-table")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .clip()
    .child(table_header("sharing-join-table-header", &columns, false));
  for row in rows {
    table = table.child(line(ctx, row, (key, unticked, person)));
  }
  // What goes up is a person's choice, as the join is: an MCP client's tick changes nothing and says why.
  let refused = rows
    .iter()
    .filter_map(|row| person.reason(ctx, &tick_id(row.task)))
    .collect::<Vec<_>>();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(table)
    .with_children(refused)
    .into()
}

/// The id of `task`'s box.
fn tick_id(task: &ReviewTask) -> String {
  format!("sharing-join-row-{}-tick", task.task.key())
}

fn line(ctx: &mut Ctx, planned: &Planned<'_>, (key, unticked, person): (&str, &Signal<Unticked>, &PersonOnly)) -> Row {
  let task = planned.task;
  let local = task.task.key().to_string();
  let id = format!("sharing-join-row-{local}");
  let mut tick = Toggle::new(
    tick_id(task),
    ToggleKind::Checkbox,
    planned.ticked,
    ctx.t_args("sharing.join.tick", [("task", local.as_str())]),
  )
  .hide_label();
  if !task.locked() {
    let (unticked, person, button, task) = (unticked.clone(), person.clone(), tick_id(task), task.task.id());
    tick = tick.on_change(move |on| {
      if !person.allows(&button) {
        return;
      }
      let mut set = unticked.get_untracked();
      if on {
        set.0.remove(&task);
      } else {
        set.0.insert(task);
      }
      unticked.set(set);
    });
  }
  let becomes = match &planned.becomes {
    Some(becomes) => Text::new(&becomes.to_string())
      .id(format!("{id}-becomes"))
      .variant(ExtraTypography::MonoKey.role())
      .color(PaletteColor::TextPrimary),
    None => Text::new(&ctx.t("sharing.join.notShared"))
      .id(format!("{id}-becomes"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted),
  };
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inline.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(cell(TICK, tick.into()))
    .child(cell(
      KEY,
      Text::new(&local)
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .into(),
    ))
    .child(cell(TASK, title(ctx, &id, (task, key)).into()))
    .child(cell(WORKFLOW, workflow(ctx, &id, task).into()))
    .child(cell(BECOMES, becomes.into()))
}

fn cell(width: ColumnWidth, content: Element) -> Row {
  let cell = Row::new().align_items(Alignment::Start).child(content);
  match width {
    ColumnWidth::Fixed(width) => cell.width(Dimension::Px(width)).flex_shrink(0.0),
    ColumnWidth::Fill => cell.flex(1.0).min_width(Dimension::Px(0.0)),
  }
}

/// The title, and under it why the task can't be unticked or the server's similar task.
fn title(ctx: &Ctx, id: &str, (task, key): (&ReviewTask, &str)) -> Column {
  let hint = if task.locked() {
    let number = task.run.map_or_else(String::new, |(number, _)| number.to_string());
    Some((
      Icon::Lock,
      ctx.t_args("sharing.join.locked", [("number", number)]),
      "locked",
    ))
  } else {
    task.similar.as_ref().map(|similar| {
      (
        Icon::Copy,
        ctx.t_args(
          "sharing.join.similar",
          [
            ("project", key),
            ("key", similar.key.as_str()),
            ("title", similar.title.as_str()),
            ("by", similar.created_by.as_str()),
          ],
        ),
        "similar",
      )
    })
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(task.task.details().title.as_str())
        .id(format!("{id}-title"))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextPrimary)
        .width(Dimension::Pct(100.0)),
    )
    .with_children(hint.map(|(glyph, words, kind)| {
      Row::new()
        .id(format!("{id}-{kind}"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Start)
        .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
        .child(Text::new(&words).variant(ExtraTypography::Note.role()).flex(1.0))
    }))
}

/// The task's workflow status over where its latest run stands (`No active run`, `Run #2 · Failed`, `Running · run
/// #3`).
fn workflow(ctx: &Ctx, id: &str, task: &ReviewTask) -> Column {
  let run = match task.run {
    None => ctx.t("sharing.join.run.none"),
    Some((number, phase)) => {
      let words = ctx.t(Status::of_phase(phase).key());
      let key = if phase.run_not_ended() || phase == TaskPhase::Queued {
        "sharing.join.run.active"
      } else {
        "sharing.join.run.ended"
      };
      ctx.t_args(key, [("phase", words.to_string()), ("number", number.to_string())])
    }
  };
  Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .child(workflow_chip(
      ctx,
      &format!("{id}-workflow"),
      task.task.organization().status(),
    ))
    .child(
      Text::new(&run)
        .id(format!("{id}-run"))
        .variant(ExtraTypography::Note.role()),
    )
}
