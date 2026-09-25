use lurq::{
  app::theme::{FormTextRole, FormTheme, PaletteColor, RadiusSize, TypographyStyle},
  layout::scrollbar::{ScrollBarPlacement, ScrollBarStyle},
  node::{color::Color, dimension::Dimension, padding::Padding},
};

use super::tokens::{
  BORDER, BUTTON_HEIGHT, CONTROL_PADDING_X, FIELD_HEIGHT, NO_TRACK, SELECT_SCROLLBAR_EDGE_INSET,
  SELECT_SCROLLBAR_END_INSET, SELECT_SCROLLBAR_MIN_THUMB, SELECT_SCROLLBAR_RADIUS, SELECT_SCROLLBAR_WIDTH, SPACE_STACK,
};

/// Field (`ZGwlv`) and button (`u1XBP`) roles for lurq's compound form controls.
pub fn form(mut form: FormTheme) -> FormTheme {
  let control_padding = || Padding {
    left: CONTROL_PADDING_X.into(),
    right: CONTROL_PADDING_X.into(),
    top: 0.0.into(),
    bottom: 0.0.into(),
  };

  form.input.height = Dimension::Px(FIELD_HEIGHT);
  form.input.padding = control_padding();
  form.input.radius = RadiusSize::Md;
  form.input.text = FormTextRole {
    typography: TypographyStyle::FieldLabel,
    color: PaletteColor::TextPrimary,
  };
  form.input.placeholder = FormTextRole {
    typography: TypographyStyle::FieldLabel,
    color: PaletteColor::TextSecondary,
  };
  form.input.caret = PaletteColor::TextPrimary;

  for role in [&mut form.button.primary, &mut form.button.secondary] {
    role.height = Dimension::Px(BUTTON_HEIGHT);
    role.padding = control_padding();
    role.radius = RadiusSize::Md;
  }

  let secondary = &mut form.button.secondary;
  secondary.background = PaletteColor::SurfaceRaised;
  secondary.border = PaletteColor::Border;
  secondary.background_hover = PaletteColor::AccentMuted;
  secondary.border_hover = PaletteColor::Border;
  secondary.background_active = PaletteColor::SurfaceRaised;
  secondary.border_active = PaletteColor::BorderStrong;

  form.checkbox.radius = RadiusSize::Sm;
  form.checkbox.checked_background = PaletteColor::Accent;
  form.checkbox.checked_border = PaletteColor::Accent;
  form
}

/// Overlay scrollbar of a select menu longer than eight options (`g3poI`).
pub fn select_menu_scrollbar() -> ScrollBarStyle {
  ScrollBarStyle {
    width: SELECT_SCROLLBAR_WIDTH,
    track_color: Color::from_hex(NO_TRACK),
    thumb_color: Color::from_hex(BORDER),
    thumb_radius: SELECT_SCROLLBAR_RADIUS,
    track_radius: SELECT_SCROLLBAR_RADIUS,
    min_thumb_length: SELECT_SCROLLBAR_MIN_THUMB,
    ..ScrollBarStyle::default()
  }
  .insets(SELECT_SCROLLBAR_EDGE_INSET, SELECT_SCROLLBAR_END_INSET)
}

/// `base` in a gutter of its own as wide as a list's side padding (`space-stack`), so the bar
/// never covers a row or its selection (changed file list `h8PZJa`).
pub fn gutter_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  let edge_inset = ((SPACE_STACK - base.width) / 2.0).max(0.0);
  ScrollBarStyle {
    placement: ScrollBarPlacement::Reserved,
    edge_inset: Some(edge_inset),
    ..base.clone()
  }
}
