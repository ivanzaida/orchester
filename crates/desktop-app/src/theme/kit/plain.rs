//! Typography roles without a color of their own, for the kit's recipes. lurq colors a part's text by the part's
//! `foreground` only where the text's role names no color, and the app's roles each carry the design's color (a
//! screen's `Text` takes its role and is colored). A recipe that sets a part's `foreground` names the role's twin
//! here, the same face, size, weight, line and spacing in no color, so the part's color is the one it shows.

use lurq::{
  app::theme::{ThemeTypography, TypographyStyle},
  layout::text_style::TextStyle,
};

use crate::theme::{ExtraTypography, SecretsText};

/// The roles a kit recipe colors through its part's `foreground`.
fn colored() -> [TypographyStyle; 11] {
  [
    TypographyStyle::Button,
    TypographyStyle::Description,
    TypographyStyle::Label,
    TypographyStyle::FieldLabel,
    ExtraTypography::FormSection.role(),
    ExtraTypography::Note.role(),
    ExtraTypography::ButtonLarge.role(),
    ExtraTypography::Initials.role(),
    ExtraTypography::MonoLabel.role(),
    ExtraTypography::MonoSmall.role(),
    SecretsText::ProjectInitial.role(),
  ]
}

/// `role`'s twin in no color.
pub fn plain(role: impl Into<TypographyStyle>) -> TypographyStyle {
  TypographyStyle::extra(format!("plain.{}", role.into().as_str()))
}

/// `typography` with the twin of every role a recipe colors.
pub(in crate::theme) fn with_plain_roles(mut typography: ThemeTypography) -> ThemeTypography {
  let no_color = TextStyle::default().color;
  for role in colored() {
    let style = TextStyle {
      color: no_color,
      ..typography.resolve(role)
    };
    typography.set(plain(role), style);
  }
  typography
}
