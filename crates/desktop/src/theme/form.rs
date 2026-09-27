use lurq::{
  app::theme::{FormTextRole, FormTheme, PaletteColor, RadiusSize, TypographyStyle},
  layout::scrollbar::{ScrollBarPlacement, ScrollBarStyle},
  node::{color::Color, dimension::Dimension, padding::Padding},
};

use super::tokens::{
  BORDER, BUTTON_HEIGHT, CONTROL_PADDING_X, FIELD_HEIGHT, GUTTER_SCROLLBAR_INSET, NO_TRACK, SCROLLBAR_MIN_THUMB,
  SCROLLBAR_RADIUS, SCROLLBAR_WIDTH, SELECT_SCROLLBAR_EDGE_INSET, SELECT_SCROLLBAR_END_INSET,
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

/// The design's scroll thumb: 4px `border`, radius 2, no track; it shows only while the content
/// overflows.
fn thumb() -> ScrollBarStyle {
  ScrollBarStyle {
    width: SCROLLBAR_WIDTH,
    track_color: Color::from_hex(NO_TRACK),
    thumb_color: Color::from_hex(BORDER),
    thumb_radius: SCROLLBAR_RADIUS,
    track_radius: SCROLLBAR_RADIUS,
    min_thumb_length: SCROLLBAR_MIN_THUMB,
    ..ScrollBarStyle::default()
  }
}

/// Overlay scrollbar of a select menu longer than eight options (`g3poI`).
pub fn select_menu_scrollbar() -> ScrollBarStyle {
  thumb().insets(SELECT_SCROLLBAR_EDGE_INSET, SELECT_SCROLLBAR_END_INSET)
}

/// Scrollbar of a viewport with a scroll gutter (design doc "Scroll gutter", 04.01 `JuVaA`): a
/// 12px gutter of its own at the right edge, as wide as a list's side padding (`space-stack`),
/// so the thumb never covers a value, a row or its selection. The gutter stays while the content
/// fits, so nothing shifts when it starts to overflow.
pub fn gutter_scrollbar() -> ScrollBarStyle {
  ScrollBarStyle {
    placement: ScrollBarPlacement::Reserved,
    ..thumb()
  }
  .insets(GUTTER_SCROLLBAR_INSET, GUTTER_SCROLLBAR_INSET)
}
