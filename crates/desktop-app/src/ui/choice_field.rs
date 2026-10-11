//! A field opening icon-led menu choices (ZIjF3 / S5LDyv), built from the shared menu masters.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  menu::{MenuItem, menu, menu_panel, separator},
  pressable::{Focus, pressable},
};
use crate::theme::{ControlHeight, ExtraSpacing, IconSize};

const MENU_MAX_HEIGHT: f32 = 280.0;

#[derive(Clone, PartialEq)]
pub struct Choice {
  pub value: String,
  pub label: Arc<str>,
  pub icon: Icon,
  pub detail: Option<Arc<str>>,
  pub separator: bool,
}

#[derive(Clone)]
pub struct ChoiceField {
  pub id: Arc<str>,
  pub name: Arc<str>,
  pub value: Signal<String>,
  pub open: Signal<bool>,
  pub choices: Vec<Choice>,
  pub width: f32,
}
impl PartialEq for ChoiceField {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.value.id() == other.value.id()
      && self.open.id() == other.open.id()
      && self.choices == other.choices
      && self.width == other.width
  }
}
impl DevtoolsInspectable for ChoiceField {}

impl ChoiceField {
  pub fn render(self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<Picker>(&self.id.clone(), self)
  }
}

struct Picker {
  anchor: ElementRef,
}
impl Component for Picker {
  type Props = ChoiceField;
  fn create(_: &mut Ctx) -> Self {
    Self {
      anchor: ElementRef::new(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChoiceField>().clone();
    let chosen = props.value.get();
    let label = props
      .choices
      .iter()
      .find(|choice| choice.value == chosen)
      .map_or(props.name.clone(), |choice| choice.label.clone());
    let open = props.open.clone();
    let trigger = Focus::Form
      .apply(pressable(move || open.update(|open| *open = !*open)))
      .id(props.id.clone())
      .ref_element(self.anchor.clone())
      .describe("a11y_name", props.name.clone())
      .describe("state", if props.open.get() { "open" } else { "closed" })
      .width(Dimension::Px(props.width))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border_inside(
        BorderSize::Sm,
        if props.open.get() {
          PaletteColor::BorderFocus
        } else {
          PaletteColor::Border
        },
      )
      .hovered_style(Style::new().border_inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .child(
        Text::new(&label)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(icon(
        if props.open.get() {
          Icon::ChevronUp
        } else {
          Icon::ChevronDown
        },
        IconSize::Md,
        PaletteColor::TextMuted,
      ));
    let mut rows = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Hair.role());
    for (index, choice) in props.choices.iter().enumerate() {
      if choice.separator {
        rows = rows.child(separator(ctx));
      }
      let (value, selected) = (props.value.clone(), choice.value.clone());
      let mut item = MenuItem::new(
        format!("{}-option-{index}", props.id),
        choice.label.clone(),
        move || value.set(selected.clone()),
      )
      .icon(choice.icon)
      .chosen(choice.value == chosen)
      .in_form();
      if let Some(detail) = &choice.detail {
        item = item.detail(detail.clone());
      }
      rows = rows.child(item.render(ctx, &props.open));
    }
    let panel = menu_panel(&format!("{}-menu", props.id))
      .width(Dimension::Px(props.width))
      .child(ScrollVertical::new(rows).max_height(Dimension::Px(MENU_MAX_HEIGHT)));
    Row::new().width(Dimension::Px(props.width)).child(trigger).child(menu(
      self.anchor.clone(),
      props.open.clone(),
      panel,
    ))
  }
}
