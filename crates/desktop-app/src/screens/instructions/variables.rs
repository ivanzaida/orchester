//! Compact variable catalog H5rZOl / zRiUA. Expanded detail uses x5WE6i.
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{Variable, VariableGroup, VariableType};
use orchester_workspace::instructions::{form::Picked, preview::Resolved};

use super::{
  panels::{Panels, Props},
  words,
};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, instructions},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};
// Variable group H5rZOl.
const GROUP_HEIGHT: f32 = 26.0;
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
  let mut rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Hair.role());
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
    let expanded = !search.is_empty() || state.groups.get().iter().any(|g| g == group_name(group));
    rows = rows.child(group_header(ctx, state, group, matching.len(), expanded));
    if !expanded {
      continue;
    }
    for variable in matching {
      rows = rows.child(if selected == variable {
        super::variable_detail::detail(ctx, props, variable, resolved)
      } else {
        row(ctx, state, variable, resolved)
      });
    }
  }
  ScrollVertical::new(rows)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .into()
}
fn group_header(ctx: &Ctx, state: &Panels, group: VariableGroup, count: usize, expanded: bool) -> Element {
  let id = group_name(group);
  let groups = state.groups.clone();
  pressable(move || {
    groups.update(|groups| {
      if groups.iter().any(|g| g == id) {
        groups.retain(|g| g != id);
      } else {
        groups.push(id.to_owned());
      }
    })
  })
  .id(format!("instruction-variable-group-{id}"))
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(GROUP_HEIGHT))
  .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
  .spacing(ExtraSpacing::Snug.role())
  .hovered_style(Style::new().background(ExtraColor::Hover.role()))
  .child(icon(
    if expanded {
      Icon::ChevronDown
    } else {
      Icon::ChevronRight
    },
    IconSize::Xs,
    PaletteColor::TextMuted,
  ))
  .child(Text::new(&group_label(ctx, group).to_uppercase()).variant(instructions::overline()))
  .child(Text::new(&count.to_string()).variant(TypographyStyle::Caption))
  .into()
}
fn row(ctx: &Ctx, state: &Panels, variable: Variable, resolved: Option<&Resolved<'_>>) -> Element {
  let choose = state.selected.clone();
  let value = summary(ctx, variable, resolved);
  pressable(move || choose.set(Picked(variable)))
    .id(format!("instruction-variable-{}", variable.name()))
    .width(Dimension::Pct(100.0))
    .rounded(ExtraRadius::Tile.role())
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Snug.role(),
    ))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .spacing(SpacingSize::Sm)
            .child(
              Text::new(variable.name())
                .variant(TypographyStyle::Mono)
                .nowrap()
                .flex(1.0)
                .min_width(Dimension::Px(0.0)),
            )
            .child(
              Text::new(&value_type(ctx, variable.value_type()))
                .variant(TypographyStyle::Caption)
                .nowrap(),
            ),
        )
        .child(
          Text::new(&value)
            .variant(instructions::variable_value())
            .width(Dimension::Pct(100.0))
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        ),
    )
    .into()
}
fn summary(ctx: &Ctx, variable: Variable, resolved: Option<&Resolved<'_>>) -> String {
  if variable == Variable::TaskAcceptanceCriteria
    && let Some(resolved) = resolved
  {
    return ctx
      .t_args(
        "instructions.criteriaCount",
        [(
          "count",
          resolved.task.task.details().acceptance_criteria.len().to_string(),
        )],
      )
      .to_string();
  }
  resolved
    .and_then(|r| r.value(variable))
    .unwrap_or_else(|| ctx.t("instructions.unavailable").to_string())
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
