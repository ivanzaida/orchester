//! Command trigger (`Command trigger` `qXoBR`): a box that opens a search when pressed, shaped like a
//! search field: a 13 `text_muted` glass 9 from its start, what it searches for in 12 `text_muted`, and the
//! shortcut that opens it in a key cap 4 from its end, 8 apart, 26 high on `surface_input` in a `border`
//! stroke, radius 6. The caller places what it opens at the trigger through the same `anchor`.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::Text,
  core::ElementRef,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  kbd::key_cap,
  pressable::pressable,
};
use crate::theme::{ControlHeight, ExtraSpacing, IconSize};

pub struct CommandTrigger {
  id: Arc<str>,
  label: Arc<str>,
  keys: Arc<str>,
  width: f32,
  anchor: ElementRef,
  press: Arc<dyn Fn() + Send + Sync>,
}

impl CommandTrigger {
  /// The trigger `id`, `width` wide, saying `label` with the shortcut `keys`, running `press`.
  pub fn new(
    id: impl Into<Arc<str>>,
    (label, keys): (impl Into<Arc<str>>, impl Into<Arc<str>>),
    width: f32,
    anchor: ElementRef,
    press: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      keys: keys.into(),
      width,
      anchor,
      press: Arc::new(press),
    }
  }
}

impl From<CommandTrigger> for Element {
  fn from(trigger: CommandTrigger) -> Self {
    let press = trigger.press.clone();
    pressable(move || press())
      .id(trigger.id.clone())
      .ref_element(trigger.anchor)
      .width(Dimension::Px(trigger.width))
      .height(Dimension::Px(ControlHeight::Picker.value()))
      .padding(Padding::new().left(ExtraSpacing::Inline.role()).right(SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .describe("a11y_name", trigger.label.clone())
      .child(icon(Icon::Search, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&trigger.label)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap()
          .flex(1.0),
      )
      .child(key_cap(&format!("{}-keys", trigger.id), &trigger.keys))
      .into()
  }
}
