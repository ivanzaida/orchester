use lurq::{
  app::theme::{
    FormButtonRole, FormButtonTheme, FormCheckboxStyle, FormFieldTheme, FormInputTheme, FormSliderStyle, FormTextRole,
    FormTheme, PaletteColor, RadiusSize, TypographyStyle,
  },
  node::{dimension::Dimension, padding::Padding, spacing_value::SpacingValue},
};

use super::ExtraColor;

/// `control_height_md`, the height of fields and buttons.
const CONTROL_HEIGHT_MD: f32 = 28.0;

/// Fields, buttons, checkboxes and sliders of the Shared components board (`ASNYj`: `Field/text`,
/// `Button/primary`, `Button/secondary`, `Checkbox/*`, `Slider`), at `control_height_md`.
pub fn form() -> FormTheme {
  FormTheme {
    field: FormFieldTheme {
      spacing: SpacingValue::from(6.0),
      label: text(TypographyStyle::FieldLabel, PaletteColor::TextSecondary),
      hint: text(TypographyStyle::Caption, PaletteColor::TextMuted),
      error: text(TypographyStyle::Caption, PaletteColor::Danger),
    },
    input: FormInputTheme {
      height: Dimension::Px(CONTROL_HEIGHT_MD),
      padding: Padding::symmetric(10.0, 4.0),
      radius: RadiusSize::Md,
      background: PaletteColor::SurfaceInput,
      border: PaletteColor::Border,
      border_focus: PaletteColor::BorderFocus,
      background_error: PaletteColor::DangerMuted,
      border_error: PaletteColor::Danger,
      text: text(TypographyStyle::Body, PaletteColor::TextPrimary),
      placeholder: text(TypographyStyle::Body, PaletteColor::TextMuted),
      caret: PaletteColor::TextPrimary,
    },
    checkbox: FormCheckboxStyle {
      background: PaletteColor::SurfaceInput,
      border: PaletteColor::BorderStrong,
      border_hover: PaletteColor::BorderFocus,
      checked_background: PaletteColor::Accent,
      checked_border: PaletteColor::Accent,
      checked_background_hover: PaletteColor::AccentHover,
      border_focus: PaletteColor::BorderFocus,
      radius: RadiusSize::Sm,
    },
    slider: FormSliderStyle {
      track: PaletteColor::BorderStrong,
      track_hover: PaletteColor::TextMuted,
      thumb: PaletteColor::Accent,
      thumb_hover: PaletteColor::AccentHover,
      thumb_border_focus: PaletteColor::BorderFocus,
    },
    button: FormButtonTheme {
      primary: FormButtonRole {
        background: PaletteColor::Accent,
        border: PaletteColor::Accent,
        background_hover: PaletteColor::AccentHover,
        border_hover: PaletteColor::AccentHover,
        background_active: PaletteColor::AccentHover,
        border_active: PaletteColor::AccentHover,
        text: text(TypographyStyle::Button, ExtraColor::OnAccent.role()),
        ..button()
      },
      secondary: FormButtonRole {
        background: PaletteColor::SurfaceRaised,
        border: PaletteColor::BorderStrong,
        background_hover: PaletteColor::Border,
        border_hover: PaletteColor::BorderStrong,
        background_active: PaletteColor::BorderStrong,
        border_active: PaletteColor::BorderStrong,
        text: text(TypographyStyle::Button, PaletteColor::TextPrimary),
        ..button()
      },
    },
  }
}

/// Size, shape and focus shared by both button roles.
fn button() -> FormButtonRole {
  FormButtonRole {
    width: Dimension::Auto,
    height: Dimension::Px(CONTROL_HEIGHT_MD),
    padding: Padding::symmetric(12.0, 0.0),
    radius: RadiusSize::Md,
    border_focus: PaletteColor::BorderFocus,
    ..FormButtonRole::secondary()
  }
}

fn text(typography: TypographyStyle, color: PaletteColor) -> FormTextRole {
  FormTextRole { typography, color }
}
