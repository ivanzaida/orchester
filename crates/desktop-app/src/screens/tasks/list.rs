//! The list's rows (`List` `ZCJb1` in `Tasks · redesign` `Lw1Gb`): a view's rows in its groups (`views::grouped`), each
//! group under its header line (`Group · Needs you` `Sv7dZ`: 34 high, 10 over its line, 8 at its sides, 8 apart: a 12
//! `text_muted` chevron that folds the group, the group's 13 icon in its tone, its name in 12/500 `text_secondary` and
//! its count in 12 `text_muted`), 2 between lines. A group folded away keeps its header. The list is as tall as its
//! lines up to the room the panel leaves, and its lines are windowed: a project's list grows past what a screen holds.

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, VirtualizedList},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::tasks::{
  attention::{NeedItem, NeedKind},
  data::ProjectTasks,
  views::{Group, Grouped},
};

use super::{
  phrase::Phrase,
  row::{ItemProps, ListItem, NeedAction, ROW_HEIGHT, RowProps},
};
use crate::{
  navigation::waiting::Destination,
  theme::{ExtraSpacing, IconSize, TasksSpacing},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
    status::{workflow_key, workflow_look},
  },
};

/// A group's header line (`Sv7dZ`).
const GROUP_HEIGHT: f32 = 34.0;

/// The groups the person folded away, in every view.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Folded(Vec<Group>);

impl DevtoolsInspectable for Folded {}

impl Folded {
  pub(super) fn holds(&self, group: Group) -> bool {
    self.0.contains(&group)
  }
}

/// Folds and unfolds the groups; the screen provides it to the header lines.
#[derive(Clone)]
pub(super) struct Folding(pub Signal<Folded>);

impl Folding {
  fn toggle(&self, group: Group) {
    self.0.update(|folded| {
      if folded.holds(group) {
        folded.0.retain(|held| *held != group);
      } else {
        folded.0.push(group);
      }
    });
  }
}

/// A group's header line, as one item of the list.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GroupLine {
  pub group: Group,
  pub count: usize,
  pub folded: bool,
}

/// What the rows are built from: the project and its tasks as read, the task a Create just added, and whether the
/// project is shared.
pub(super) struct Source<'a> {
  pub project: ProjectId,
  pub tasks: &'a ProjectTasks,
  pub needs: &'a HashMap<orchester_core::id::TaskId, NeedItem>,
  pub highlight: Option<&'a str>,
  pub shared: bool,
}

/// The list's items: each group's header line and, unless it is folded, its rows; a flat view's rows alone.
pub(super) fn items(ctx: &Ctx, source: &Source<'_>, grouped: &[Grouped], folded: &Folded) -> Vec<ItemProps> {
  let now = Timestamp::now();
  let mut items = Vec::new();
  for part in grouped {
    if let Some(group) = part.group {
      let folded = folded.holds(group);
      items.push(ItemProps::Group(GroupLine {
        group,
        count: part.rows.len(),
        folded,
      }));
      if folded {
        continue;
      }
    }
    items.extend(part.rows.iter().map(|row| {
      let need = source.needs.get(&row.id);
      ItemProps::Task(Box::new(RowProps {
        project: source.project,
        phrase: Phrase::of(ctx, row, need, source.tasks),
        action: need.map(|need| action(ctx, source.project, need)),
        age: age(ctx, now, row.created_at),
        highlighted: source.highlight == Some(&*row.key),
        shared: source.shared,
        row: row.clone(),
      }))
    }));
  }
  items
}

/// What a waiting task's action says and opens: where `need` is decided.
fn action(ctx: &Ctx, project: ProjectId, need: &NeedItem) -> NeedAction {
  NeedAction {
    label: message_text(ctx, &need.kind.action()),
    glyph: match need.kind {
      NeedKind::Approval => Icon::ShieldCheck,
      NeedKind::Question => Icon::MessageCircle,
      NeedKind::PlanReview => Icon::Map,
      _ => Icon::ArrowUpRight,
    },
    destination: Destination::of(project, need).map_err(|reason| message_text(ctx, &reason)),
  }
}

/// How long ago `created` was at `now`, in its largest whole unit: `now`, `12m`, `2h`, `3d`.
fn age(ctx: &Ctx, now: Timestamp, created: Timestamp) -> Arc<str> {
  let minutes = (now.unix_millis() - created.unix_millis()).max(0) / 60_000;
  let (key, count) = match minutes {
    0 => return ctx.t("tasks.age.now"),
    1..60 => ("tasks.age.minutes", minutes),
    60..1440 => ("tasks.age.hours", minutes / 60),
    _ => ("tasks.age.days", minutes / 1440),
  };
  ctx.t_args(key, [("count", count.to_string())])
}

/// The height an item takes with the gap under it.
fn height(item: &ItemProps) -> f32 {
  let line = match item {
    ItemProps::Group(_) => GROUP_HEIGHT,
    ItemProps::Task(_) => ROW_HEIGHT,
  };
  line + ExtraSpacing::Tight.value()
}

/// How far down the list the row of `key` starts, if it is among `items`.
pub(super) fn offset_of(items: &[ItemProps], key: &str) -> Option<f32> {
  let index = items
    .iter()
    .position(|item| matches!(item, ItemProps::Task(props) if *props.row.key == *key))?;
  Some(items[..index].iter().map(height).sum())
}

/// The items, windowed and scrolled where `scroll` says; `list_key` is new for another view, search, filter or sort: a
/// list keeps its top row in place while its rows change, which a new list must not.
pub(super) fn list(ctx: &mut Ctx, items: Arc<Vec<ItemProps>>, (scroll, list_key): (ScrollState, String)) -> Element {
  let total: f32 = items.iter().map(height).sum();
  let lines = VirtualizedList::new_shared(ctx, items)
    .list_key(list_key)
    .with_scroll_state(scroll)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(total))
    .mount_keyed::<ListItem, _, _, _>(item_key, Clone::clone);
  Column::new()
    .id("tasks-list")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(total))
    .child(lines)
    .into()
}

fn item_key(item: &ItemProps) -> String {
  match item {
    ItemProps::Group(line) => format!("group-{}", line.group.name()),
    ItemProps::Task(props) => props.row.key.to_string(),
  }
}

/// A group's header line, which folds the group away or unfolds it.
pub(super) fn group_line(ctx: &mut Ctx, line: &GroupLine) -> Element {
  let folding = ctx
    .use_context::<Folding>()
    .expect("the Tasks list provides its groups' folding");
  let group = line.group;
  let id = format!("tasks-group-{}", group.name());
  let (glyph, color) = look(group);
  let label = ctx.t(label_key(group));
  pressable(move || folding.toggle(group))
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(
      Padding::new()
        .top(TasksSpacing::GroupTop.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("a11y_name", ctx.t_args("tasks.groups.toggle", [("group", &*label)]))
    .describe("state", if line.folded { "folded" } else { "open" })
    .child(icon(
      if line.folded {
        Icon::ChevronRight
      } else {
        Icon::ChevronDown
      },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(icon(glyph, IconSize::Base, color).id(format!("{id}-icon")))
    .child(
      Text::new(&label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Text::new(&line.count.to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// A group's icon and its tone: Active's by what happens next, All's by workflow status.
const fn look(group: Group) -> (Icon, PaletteColor) {
  match group {
    Group::NeedsYou => (Icon::Hand, PaletteColor::Warning),
    Group::Running => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    Group::Queued => (Icon::Clock3, PaletteColor::TextMuted),
    Group::ReadyToStart => (Icon::Circle, PaletteColor::TextSecondary),
    Group::Stopped => (Icon::CirclePause, PaletteColor::TextMuted),
    Group::Finished => (Icon::CircleCheck, PaletteColor::Success),
    Group::Workflow(status) => workflow_look(status),
  }
}

const fn label_key(group: Group) -> &'static str {
  match group {
    Group::NeedsYou => "tasks.groups.needsYou",
    Group::Running => "tasks.groups.running",
    Group::Queued => "tasks.groups.queued",
    Group::ReadyToStart => "tasks.groups.ready",
    Group::Stopped => "tasks.groups.stopped",
    Group::Finished => "tasks.groups.finished",
    Group::Workflow(status) => workflow_key(status),
  }
}
