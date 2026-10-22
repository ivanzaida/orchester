//! A resource of the source to select (`Import resource row` `h0FH27`): its checkbox, its name over what it is,
//! what imports it along when it is not selected itself, the content its copy takes, and what it refers to.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ResourceId, import::Pick};

use super::{
  Picked,
  select::{DEPENDS_WIDTH, VERSION_WIDTH},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::SelectField,
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

/// A pick as the version select's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PickValue(Pick);

impl DevtoolsInspectable for PickValue {}

#[derive(Clone)]
pub(super) struct ResourceRowProps {
  pub id: ResourceId,
  pub name: Arc<str>,
  /// What it is: "Pipeline · 3 steps · 6 dependencies".
  pub summary: Arc<str>,
  /// "Included with Preview release", while a selected resource's import takes it along.
  pub included_with: Option<Arc<str>>,
  /// What it refers to; `None` for nothing.
  pub depends_on: Option<Arc<str>>,
  /// The contents its copy can take, labelled.
  pub picks: Vec<(Pick, Arc<str>)>,
  pub pick: Pick,
  pub selected: bool,
  /// The page's selection, which the row changes.
  pub picked: Signal<Picked>,
}

impl PartialEq for ResourceRowProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.summary == other.summary
      && self.included_with == other.included_with
      && self.depends_on == other.depends_on
      && self.picks == other.picks
      && self.pick == other.pick
      && self.selected == other.selected
  }
}

impl DevtoolsInspectable for ResourceRowProps {}

pub(super) fn mount(ctx: &mut Ctx, props: ResourceRowProps) -> Element {
  ctx.mount_keyed::<ResourceRow>(&format!("import-resource-{}", props.id), props)
}

struct ResourceRow {
  pick: Signal<PickValue>,
}

impl Component for ResourceRow {
  type Props = ResourceRowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ResourceRowProps>().clone();
    let pick = ctx.signal(PickValue(props.pick));
    let (picked, id) = (props.picked, props.id);
    ctx.watch(&pick, move |PickValue(chosen)| {
      let chosen = *chosen;
      picked.update(|Picked(picked)| {
        if let Some(entry) = picked.iter_mut().find(|(picked, _)| *picked == id) {
          entry.1 = chosen;
        }
      });
    });
    Self { pick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ResourceRowProps>().clone();
    let id = props.id;
    Row::new()
      .id(format!("import-resource-{id}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().right(SpacingSize::Sm))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .describe("selected", if props.selected { "true" } else { "false" })
      .child(self.item(&props))
      .child(self.version(ctx, &props))
      .child(depends_on(ctx, &props))
  }
}

impl ResourceRow {
  /// The checkbox, the name over what it is, and what takes it along (`Import item row` `zNzKN`).
  fn item(&self, props: &ResourceRowProps) -> Row {
    let (picked, id, pick) = (props.picked.clone(), props.id, self.pick.clone());
    let check = Toggle::new(
      format!("import-check-{id}"),
      ToggleKind::Checkbox,
      props.selected,
      props.name.clone(),
    )
    .hide_label()
    .on_change(move |on| {
      picked.update(|Picked(picked)| {
        picked.retain(|(picked, _)| *picked != id);
        if on {
          picked.push((id, pick.get_untracked().0));
        }
      });
    });
    Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
      .spacing(ExtraSpacing::Loose.role())
      .rounded(RadiusSize::Md)
      .child(check)
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&props.name)
              .id(format!("import-name-{id}"))
              .variant(TypographyStyle::Mono)
              .nowrap(),
          )
          .child(
            Text::new(&props.summary)
              .id(format!("import-summary-{id}"))
              .width(Dimension::Pct(100.0))
              .variant(ExtraTypography::Note.role())
              .color(PaletteColor::TextSecondary),
          ),
      )
      .with_children(props.included_with.as_deref().map(|included| {
        Text::new(included)
          .id(format!("import-included-{id}"))
          .variant(TypographyStyle::Caption)
          .nowrap()
      }))
  }

  /// The content the copy takes (`Version` in `h0FH27`).
  fn version(&self, ctx: &Ctx, props: &ResourceRowProps) -> Element {
    let options: Vec<SelectOption<PickValue>> = props
      .picks
      .iter()
      .map(|(pick, label)| SelectOption::new(PickValue(*pick), label.clone()))
      .collect();
    Row::new()
      .width(Dimension::Px(VERSION_WIDTH))
      .child(
        SelectField::new(
          format!("import-version-{}", props.id),
          props.name.clone(),
          self.pick.clone(),
          options,
        )
        .render(ctx),
      )
      .into()
  }
}

/// What the resource refers to, or None (`Depends on` `zoz5T`).
fn depends_on(ctx: &Ctx, props: &ResourceRowProps) -> Row {
  let (text, color) = match &props.depends_on {
    Some(names) => (names.clone(), PaletteColor::TextSecondary),
    None => (ctx.t("import.dependsOn.none"), PaletteColor::TextMuted),
  };
  Row::new()
    .width(Dimension::Px(DEPENDS_WIDTH))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Link, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&text)
        .id(format!("import-depends-{}", props.id))
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .variant(TypographyStyle::Description)
        .color(color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}
