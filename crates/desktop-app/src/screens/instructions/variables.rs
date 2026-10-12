//! Variable catalog and its expanded detail jNHQ3; unavailable values stay explicit (J7EgbM).
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::resource::{Variable, VariableGroup, VariableType};
use orchester_workspace::instructions::{
  form::{Events, Picked},
  preview::Resolved,
};

use super::panels::{Panels, Props};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    pressable::pressable,
  },
};
pub fn catalog(ctx: &Ctx, state: &Panels, props: &Props, resolved: Option<&Resolved<'_>>) -> Element {
  let search = state.search.get().to_lowercase();
  let selected = state.selected.get().0;
  let mut rows = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Hair.role());
  for group in VariableGroup::ALL {
    let id = group_name(group);
    let matching = Variable::ALL
      .into_iter()
      .filter(|v| v.group() == group)
      .filter(|v| v.name().contains(&search) || description(ctx, *v).to_lowercase().contains(&search))
      .collect::<Vec<_>>();
    if matching.is_empty() {
      continue;
    }
    let groups = state.groups.clone();
    let head = pressable(move || {
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
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Text::new(
        &ctx
          .t(&format!("instructions.group{}", {
            let mut c = id.chars();
            c.next().expect("group names are nonempty").to_uppercase().to_string() + c.as_str()
          }))
          .to_uppercase(),
      )
      .variant(instructions::overline())
      .flex(1.0),
    )
    .child(Text::new(&matching.len().to_string()).variant(TypographyStyle::Caption));
    rows = rows.child(head);
    if search.is_empty() && !state.groups.get().iter().any(|g| g == id) {
      continue;
    }
    for variable in matching {
      if selected == variable {
        rows = rows.child(detail(ctx, props, variable, resolved));
      } else {
        let choose = state.selected.clone();
        let value = resolved
          .and_then(|r| r.value(variable))
          .unwrap_or_else(|| ctx.t("instructions.unavailable").to_string());
        rows = rows.child(
          pressable(move || choose.set(Picked(variable)))
            .id(format!("instruction-variable-{}", variable.name()))
            .width(Dimension::Pct(100.0))
            .rounded(ExtraRadius::Tile.role())
            .padding(ExtraSpacing::Loose.role())
            .hovered_style(Style::new().background(ExtraColor::Hover.role()))
            .child(
              Column::new()
                .width(Dimension::Pct(100.0))
                .spacing(ExtraSpacing::Tight.role())
                .child(
                  Row::new()
                    .width(Dimension::Pct(100.0))
                    .child(Text::new(variable.name()).variant(TypographyStyle::Mono).flex(1.0))
                    .child(Text::new(&value_type(ctx, variable.value_type())).variant(TypographyStyle::Caption)),
                )
                .child(Text::new(&value).variant(TypographyStyle::Caption)),
            ),
        );
      }
    }
  }
  rows
    .child(Text::new(&ctx.t("instructions.missingNote")).variant(TypographyStyle::Caption))
    .into()
}
fn detail(ctx: &Ctx, props: &Props, variable: Variable, resolved: Option<&Resolved<'_>>) -> Element {
  let value = resolved.and_then(|r| r.value(variable));
  let input = props.input.clone();
  let expression = format!("{{{{{}}}}}", variable.name());
  let insert = ActionButton::new(
    "instruction-variable-insert",
    ButtonKind::Secondary,
    ctx.t_args("instructions.insert", [("expression", expression.as_str())]),
  )
  .on_click(move || input.report(input.insert(&expression)));
  let mut detail = Column::new()
    .id("instruction-variable-detail")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Inline.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(Text::new(variable.name()).variant(TypographyStyle::Mono))
    .child(Text::new(&value_type(ctx, variable.value_type())).variant(TypographyStyle::Caption))
    .child(Text::new(&description(ctx, variable)).variant(TypographyStyle::Description))
    .child(Text::new(&ctx.t("instructions.valueAtStart")).variant(instructions::overline()))
    .child(
      Text::new(
        &value
          .clone()
          .unwrap_or_else(|| ctx.t("instructions.unavailable").to_string()),
      )
      .id("instruction-variable-value")
      .variant(TypographyStyle::Description)
      .selectable(true),
    );
  if value.is_none() {
    let events = props.events.clone();
    detail = detail.child(Text::new(&ctx.t("instructions.noEmptyValue")).variant(TypographyStyle::Caption));
    if variable == Variable::RunRevision {
      detail = detail
        .child(
          ActionButton::new(
            "instruction-variable-resume",
            ButtonKind::Secondary,
            ctx.t("instructions.injectResume"),
          )
          .on_click(move || events.set(Events::Resume)),
        )
        .child(Text::new(&ctx.t("instructions.noFallback")).variant(TypographyStyle::Caption));
    }
  }
  detail.child(insert).into()
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
fn value_type(ctx: &Ctx, kind: VariableType) -> Arc<str> {
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
fn description(ctx: &Ctx, variable: Variable) -> Arc<str> {
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
