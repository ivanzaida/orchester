//! What a bulk organization change did (02.03C `DWqjc`): one line for the tasks it changed, in
//! ORC-39's words ("Status set to Todo for 3 tasks."), and one line for each task it could not
//! change, in the problem style.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::Element,
};

use super::bulk::{Mutation, Outcomes};
use crate::{
  tasks::{
    data::ProjectTasks,
    organization::{priority_copy, workflow_copy},
  },
  ui::{
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// The outcome of the latest bulk change under the organization group, or that one is under way.
pub(super) fn outcome(ctx: &Ctx, tasks: &ProjectTasks, outcomes: Option<&Outcomes>, pending: bool) -> Element {
  let mut column = Column::new().spacing(SpacingSize::Sm);
  if pending {
    return column
      .child(Text::new(&ctx.t("tasks.workflow.saving")).variant(TypographyStyle::Caption))
      .into();
  }
  let Some(outcomes) = outcomes else {
    return column.into();
  };
  let changed = outcomes.results.iter().filter(|(_, result)| result.is_ok()).count();
  if changed > 0 {
    column = column.child(summary(ctx, tasks, &outcomes.mutation, changed));
  }
  for (id, result) in &outcomes.results {
    let Err(error) = result else {
      continue;
    };
    let key = tasks
      .tasks
      .iter()
      .find(|task| task.id() == *id)
      .map_or_else(|| id.to_string(), |task| task.key().to_string());
    let line = field_error(&ctx.t_args(
      "tasks.selection.failed",
      [("task", key.as_str()), ("reason", &message_text(ctx, error))],
    ));
    column = column.child(line.id(format!("bulk-outcome-{id}")));
  }
  column.into()
}

/// "Status set to Todo for 3 tasks.": check 12 and Inter 11, both muted.
fn summary(ctx: &Ctx, tasks: &ProjectTasks, mutation: &Mutation, count: usize) -> Element {
  let one = count == 1;
  let count_text = count.to_string();
  let count = ("count", count_text.as_str());
  let text = match mutation {
    Mutation::Status(status) => ctx.t_args(
      plural(one, "tasks.selection.statusSet.one", "tasks.selection.statusSet.other"),
      [("status", ctx.t(workflow_copy(*status).1).as_ref()), count],
    ),
    Mutation::Priority(priority) => ctx.t_args(
      plural(
        one,
        "tasks.selection.prioritySet.one",
        "tasks.selection.prioritySet.other",
      ),
      [("priority", ctx.t(priority_copy(*priority).1).as_ref()), count],
    ),
    Mutation::AddTags(ids) => ctx.t_args(
      plural(one, "tasks.selection.tagsAdded.one", "tasks.selection.tagsAdded.other"),
      [("tags", tag_names(ctx, tasks, ids).as_str()), count],
    ),
    Mutation::RemoveTags(ids) => ctx.t_args(
      plural(
        one,
        "tasks.selection.tagsRemoved.one",
        "tasks.selection.tagsRemoved.other",
      ),
      [("tags", tag_names(ctx, tasks, ids).as_str()), count],
    ),
  };
  Row::new()
    .id("bulk-outcome")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Check, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(TypographyStyle::Caption))
    .into()
}

fn tag_names(ctx: &Ctx, tasks: &ProjectTasks, ids: &[orchester_core::id::TagId]) -> String {
  tasks
    .tags
    .iter()
    .filter(|tag| ids.contains(&tag.id()))
    .map(|tag| tag.name().as_str().to_owned())
    .collect::<Vec<_>>()
    .join(&ctx.t("common.listSeparator"))
}

const fn plural(one: bool, singular: &'static str, other: &'static str) -> &'static str {
  if one { singular } else { other }
}
