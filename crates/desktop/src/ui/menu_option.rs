//! Organization menu options (gNExS, l7kBSq, IOWwP): full-width rows and trailing checks.
use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop, surface},
};

pub fn menu_option(label: &str, selected: bool, enabled: bool, on_click: impl Fn() + Send + Sync + 'static) -> Element {
  let content = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(Text::new(label).variant(TypographyStyle::Button).flex(1.0))
    .with_children(selected.then(|| icon(Icon::Check, IconSize::Small, PaletteColor::TextPrimary)));
  let mut button = surface()
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(super::field::SELECT_OPTION_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .align_items(Alignment::Center)
    .background(if selected {
      PaletteColor::AccentMuted
    } else {
      PaletteColor::SurfaceRaised
    })
    .child(content);
  if enabled {
    button = keyboard_stop(button, Ring::Inset)
      .cursor(lurq::node::CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| on_click());
  } else {
    button = button.opacity(crate::theme::DISABLED_OPACITY);
  }
  button.into()
}
