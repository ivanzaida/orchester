//! The variable catalog (02d `JXvc0`): a group per kind of variable, its head over its rows (`Task` `mqafh`), each
//! row the variable's `{{name}}`, what it is and its type at the end, or why it has no value here in `warning`
//! (`QxV66`); the chosen row is filled and its detail follows the group (`Detail` `M6fbYX`).
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{Variable, VariableGroup, VariableType};
use orchester_workspace::instructions::{form::Picked, preview::Resolved};

use super::{
  panels::{Panels, Props},
  words,
};
use crate::{
  screens::hub::parts::{self, Entry},
  theme::{HubSpacing, HubText, instructions},
  ui::pressable::Press,
};
/// What the detail says it shows: the value at the injection event, for the task and role chosen (`x9Cv4`).
pub fn context(ctx: &Ctx, props: &Props, resolved: &Resolved<'_>) -> Element {
  Text::new(&ctx.t_args(
    "instructions.variableContext",
    [
      ("event", words::event(ctx, props.events.get()).to_uppercase()),
      ("task", resolved.task.task.key().to_string()),
      ("role", resolved.role().to_owned()),
    ],
  ))
  .id("instruction-variable-context")
  .variant(instructions::overline())
  .nowrap()
  .into()
}
pub fn catalog(ctx: &Ctx, state: &Panels, props: &Props, resolved: Option<&Resolved<'_>>) -> Element {
  let search = state.search.get().to_lowercase();
  let selected = state.selected.get().0;
  let mut groups = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::SideSections.role());
  for group in VariableGroup::ALL {
    let mut matching = Variable::ALL
      .into_iter()
      .filter(|v| v.group() == group)
      .filter(|v| v.name().contains(&search) || description(ctx, *v).to_lowercase().contains(&search))
      .collect::<Vec<_>>();
    if !search.is_empty() {
      matching.sort_by_key(|variable| *variable != selected);
    }
    if matching.is_empty() {
      continue;
    }
    let id = format!("instruction-variable-group-{}", group_name(group));
    let rows = matching.iter().map(|variable| row(ctx, state, *variable, resolved));
    let head = parts::head(&id, &group_label(ctx, group), None, None);
    let mut shown = parts::group(&id, head, parts::boxed(&format!("{id}-rows"), rows));
    if matching.contains(&selected) {
      let context = resolved.map(|resolved| context(ctx, props, resolved));
      shown = shown.child(super::variable_detail::detail(ctx, props, selected, resolved, context));
    }
    groups = groups.child(shown);
  }
  ScrollVertical::new(groups)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .into()
}
fn row(ctx: &Ctx, state: &Panels, variable: Variable, resolved: Option<&Resolved<'_>>) -> Element {
  let choose = state.selected.clone();
  let missing = resolved.is_some_and(|resolved| resolved.value(variable).is_none());
  let end = if missing {
    Text::new(&ctx.t(if variable == Variable::RunRevision {
      "instructions.unavailableCommit"
    } else {
      "instructions.unavailable"
    }))
    .variant(HubText::Hint.role())
    .color(PaletteColor::Warning)
    .nowrap()
  } else {
    parts::quiet(&value_type(ctx, variable.value_type()))
  };
  Entry::new(
    format!("instruction-variable-{}", variable.name()),
    format!("{{{{{}}}}}", variable.name()),
  )
  .code()
  .hint(description(ctx, variable))
  .end(end)
  .chosen(state.selected.get().0 == variable)
  .in_place()
  .press(Press::run(move || choose.set(Picked(variable))))
  .into()
}
pub(super) fn group_label(ctx: &Ctx, group: VariableGroup) -> Arc<str> {
  ctx.t(match group {
    VariableGroup::Task => "instructions.groupTask",
    VariableGroup::Project => "instructions.groupProject",
    VariableGroup::Agent => "instructions.groupAgent",
    VariableGroup::Pipeline => "instructions.groupPipeline",
    VariableGroup::Run => "instructions.groupRun",
    VariableGroup::Step => "instructions.groupStep",
  })
}
pub fn group_name(group: VariableGroup) -> &'static str {
  match group {
    VariableGroup::Task => "task",
    VariableGroup::Project => "project",
    VariableGroup::Agent => "agent",
    VariableGroup::Pipeline => "pipeline",
    VariableGroup::Run => "run",
    VariableGroup::Step => "step",
  }
}
pub(super) fn value_type(ctx: &Ctx, kind: VariableType) -> Arc<str> {
  ctx.t(match kind {
    VariableType::String => "instructions.typeString",
    VariableType::StringList => "instructions.typeStringList",
    VariableType::AttachmentList => "instructions.typeAttachmentList",
    VariableType::Timestamp => "instructions.typeTimestamp",
    VariableType::Repository => "instructions.typeRepository",
    VariableType::Integer => "instructions.typeInteger",
    VariableType::Tokens => "instructions.typeTokens",
  })
}
pub(super) fn description(ctx: &Ctx, variable: Variable) -> Arc<str> {
  let name = variable
    .name()
    .split(['.', '_'])
    .enumerate()
    .map(|(i, part)| {
      if i == 0 {
        part.to_owned()
      } else {
        let mut chars = part.chars();
        chars
          .next()
          .map_or_else(String::new, |c| c.to_uppercase().to_string() + chars.as_str())
      }
    })
    .collect::<String>();
  ctx.t(&format!("instructions.catalog.{name}"))
}
