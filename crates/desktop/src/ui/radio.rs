//! Compact radio option (`ihdDb`): a radio mark beside its label, and optionally a mono detail after
//! it (a preset's host, 04.H1). A form control: Tab reaches an available option and Enter or Space
//! chooses it. An option that cannot be chosen is faded and not focusable; the caller says why.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Text},
  layout::Alignment,
  node::{CursorIcon, Element},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop},
};
use crate::theme::{ExtraTypography, UNAVAILABLE_OPTION_OPACITY};

pub struct Radio {
  id: &'static str,
  label: Arc<str>,
  detail: Option<Arc<str>>,
  selected: bool,
}

impl Radio {
  pub fn new(id: &'static str, label: Arc<str>, selected: bool) -> Self {
    Self {
      id,
      label,
      detail: None,
      selected,
    }
  }

  /// A value in mono after the label, e.g. the host a preset reaches.
  pub fn detail(mut self, detail: Arc<str>) -> Self {
    self.detail = Some(detail);
    self
  }

  fn row(&self) -> Button {
    let (glyph, color) = if self.selected {
      (Icon::CircleDot, PaletteColor::TextPrimary)
    } else {
      (Icon::Circle, PaletteColor::TextSecondary)
    };
    Button::empty()
      .button()
      .id(self.id)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .describe("selected", if self.selected { "true" } else { "false" })
      .child(icon(glyph, IconSize::Event, color.clone()))
      .child(Text::new(&self.label).variant(TypographyStyle::FieldLabel).color(color))
      .with_children(self.detail.as_deref().map(|detail| {
        Text::new(detail)
          .variant(ExtraTypography::MonoValue.role())
          .color(PaletteColor::TextSecondary)
      }))
  }

  /// The option, chosen by a press.
  pub fn render(self, choose: impl Fn() + Send + Sync + 'static) -> Element {
    keyboard_stop(self.row(), Ring::Outer)
      .cursor(CursorIcon::Pointer)
      .describe("state", "available")
      .on_click(move |_| choose())
      .into()
  }

  /// The option faded, not focusable and not choosable.
  pub fn unavailable(self) -> Element {
    self
      .row()
      .focusable(false)
      .cursor(CursorIcon::NotAllowed)
      .opacity(UNAVAILABLE_OPTION_OPACITY)
      .describe("state", "unavailable")
      .into()
  }
}
