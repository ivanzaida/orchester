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
use crate::theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize};

const MENU_MAX_HEIGHT: f32 = 280.0;

#[derive(Clone, PartialEq)]
pub struct Choice {
  pub value: String,
  pub label: Arc<str>,
  pub icon: Icon,
  pub detail: Option<Arc<str>>,
  pub separator: bool,
  /// The detail says what is wrong with the choice, in `warning`.
  pub warning: bool,
  /// What the field shows while the choice is chosen; its label when `None`.
  pub trigger: Option<Arc<str>>,
  /// Why the choice cannot be made now: the row is faded, ends in a lock and wraps this as its detail.
  pub unavailable: Option<Arc<str>>,
}

#[derive(Clone)]
pub struct ChoiceField {
  pub id: Arc<str>,
  pub name: Arc<str>,
  pub value: Signal<String>,
  pub open: Signal<bool>,
  pub choices: Vec<Choice>,
  pub width: f32,
  /// The field shows the chosen choice's icon before its words.
  pub icon_in_field: bool,
  /// What the field says while no choice is chosen; its name when `None`. With no choices at all it does not open.
  pub placeholder: Option<Arc<str>>,
  /// The choices with a value are identifiers (model ids), in Geist Mono in the field and the menu.
  pub mono: bool,
}
impl PartialEq for ChoiceField {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.value.id() == other.value.id()
      && self.open.id() == other.open.id()
      && self.choices == other.choices
      && self.width == other.width
      && self.icon_in_field == other.icon_in_field
      && self.placeholder == other.placeholder
      && self.mono == other.mono
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
    let trigger = self.trigger(&props, &chosen);
    let mut rows = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Hair.role());
    for (index, choice) in props.choices.iter().enumerate() {
      if choice.separator {
        rows = rows.child(separator(ctx));
      }
      let id = format!("{}-option-{index}", props.id);
      let item = match &choice.unavailable {
        Some(reason) => MenuItem::unavailable(id, choice.label.clone(), reason.clone()),
        None => {
          let (value, selected) = (props.value.clone(), choice.value.clone());
          MenuItem::new(id, choice.label.clone(), move || value.set(selected.clone()))
        }
      };
      let item = if props.mono && !choice.value.is_empty() {
        item.mono()
      } else {
        item
      };
      let mut item = item.icon(choice.icon).chosen(choice.value == chosen).in_form();
      if let Some(detail) = &choice.detail {
        item = item.detail(detail.clone());
      }
      if choice.warning {
        item = item.warning_detail();
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

impl Picker {
  /// The field: the chosen choice's words (and icon), and the chevron.
  fn trigger(&self, props: &ChoiceField, chosen: &str) -> lurq::components::Button {
    let current = props.choices.iter().find(|choice| choice.value == chosen);
    let label = current.map_or_else(
      || props.placeholder.clone().unwrap_or_else(|| props.name.clone()),
      |choice| choice.trigger.clone().unwrap_or_else(|| choice.label.clone()),
    );
    let leading = current
      .filter(|_| props.icon_in_field)
      .map(|choice| icon(choice.icon, IconSize::Md, PaletteColor::TextSecondary));
    let (open, empty) = (props.open.clone(), props.choices.is_empty());
    Focus::Keyboard
      .apply(pressable(move || {
        if !empty {
          open.update(|open| *open = !*open);
        }
      }))
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
      .with_children(leading)
      .child(
        Text::new(&label)
          .variant(
            if props.mono && current.is_some_and(|choice| !choice.value.is_empty()) {
              ExtraTypography::PropertyMono.role()
            } else {
              TypographyStyle::Description
            },
          )
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
      ))
  }
}
