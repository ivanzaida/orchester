use lurq::{
  app::theme::{
    FormButtonRole, FormButtonTheme, FormCheckboxStyle, FormFieldTheme, FormInputTheme, FormSliderStyle, FormTextRole,
    FormTheme, PaletteColor, RadiusSize, SpacingSize, TypographyStyle,
  },
  node::{dimension::Dimension, padding::Padding},
};

use super::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography};

/// Fields, buttons, checkboxes and sliders of the Shared components board (`ASNYj`: `Field/text`
/// `JFXZg`, `Button/primary` `yJ2jo`, `Button/secondary` `Dswcx`, `Checkbox/*`, `Slider`), at
/// `control_height_md`. A field's value is 12 `text_primary` (`JQkW2`), its placeholder 12 `text_muted`
/// (`X75zC`); a field with an error keeps its fill and takes a `danger` stroke, with the error in 11.5
/// `danger` under it (`CqBFL`).
pub fn form() -> FormTheme {
  FormTheme {
    field: FormFieldTheme {
      spacing: ExtraSpacing::Snug.role().into(),
      label: text(TypographyStyle::FieldLabel, PaletteColor::TextSecondary),
      hint: text(TypographyStyle::Caption, PaletteColor::TextMuted),
      error: text(ExtraTypography::Note.role(), PaletteColor::Danger),
    },
    input: FormInputTheme {
      height: Dimension::Px(ControlHeight::Md.value()),
      padding: Padding::horizontal(ExtraSpacing::Loose.role()),
      radius: RadiusSize::Md,
      background: PaletteColor::SurfaceInput,
      border: PaletteColor::Border,
      border_focus: PaletteColor::BorderFocus,
      background_error: PaletteColor::SurfaceInput,
      border_error: PaletteColor::Danger,
      text: text(TypographyStyle::Description, PaletteColor::TextPrimary),
      placeholder: text(TypographyStyle::Description, PaletteColor::TextMuted),
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

/// Size, shape and focus shared by both button roles: 28 high, 12 at each side (`yJ2jo`).
fn button() -> FormButtonRole {
  FormButtonRole {
    width: Dimension::Auto,
    height: Dimension::Px(ControlHeight::Md.value()),
    padding: Padding::horizontal(SpacingSize::Md),
    radius: RadiusSize::Md,
    border_focus: PaletteColor::BorderFocus,
    ..FormButtonRole::secondary()
  }
}

fn text(typography: TypographyStyle, color: PaletteColor) -> FormTextRole {
  FormTextRole { typography, color }
}
