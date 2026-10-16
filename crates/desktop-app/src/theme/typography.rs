use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, ThemeTypography, TypographyStyle},
  layout::text_style::{FontFeature, FontFeatures, FontWeight, TextStyle},
  node::color::Color,
};

use super::{
  ExtraColor,
  fonts::{ICONS, MONO, SANS},
};

/// Design text styles without a built-in lurq role, registered as `TypographyStyle::Extra`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraTypography {
  /// `type/display` 28/600, tracking -0.5: the largest headline of a screen.
  Display,
  /// 13/500 `text_primary`, 1.3 (the design's own line, 17): an active page tab (`loUWZ`), a notice's
  /// title (`OswfE`), a table row's title (`LJ8gd`).
  Strong,
  /// 11.5 `text_muted`, 1.4: why a control is unavailable (`Cvxzc` in `QR1ER`), a field's error
  /// (`rCoJS`, in `danger`), a run line (`v9xIKo`).
  Note,
  /// 11.5/500, 1.4: a compact status label, as a table row's run line says it (`GCsFT` in `AgVPe`).
  NoteStrong,
  /// 12 `text_secondary`, 1.45: a notice's body (`xPTrL`), a form row's description (`m1daL`), a
  /// detail row's value (`T7A7ah`, in `text_primary`).
  Prose,
  /// 12.5 `text_secondary`, 1.5: what an empty state explains (`bKD3G` in `LeIOm`).
  Lead,
  /// 10.5/500 `text_muted`, tracking 0.4: a table column's heading (`BRFPt` in `FjOYw`).
  ColumnHeading,
  /// Mono 11.5 `text_muted`: a task key in a table row (`c6k6c` in `XEQuo`).
  MonoKey,
  /// Mono 11 `text_muted`: a menu item's shortcut (`SYiST` in `mAHGP`).
  MonoNote,
  /// Mono 10 `text_secondary`: a shortcut in a tooltip (`n75PTn`) or a key cap (`ylWgq`).
  MonoSmall,
  /// Mono 11/500 `text_secondary`, 1.35: a path in a tooltip (`mF90E` in `C9Phsz`).
  MonoLabel,
  /// 9/600 `text_primary`: a person's initials in an avatar (`dWnjG` in `x5rV0`).
  Initials,
  /// 12.5/500 `text_primary`: an option card's title (`ldQGh` in `Option card/off` `Imh8O`).
  OptionTitle,
  /// 12 `text_secondary`, 1.3: what an inspector's property is (`X7na6R` in `Inspector row/value` `vhBEA`); its
  /// value in `text_primary` (`UtQHq`).
  Property,
  /// 12/600 `text_primary`: an inspector section's title (`e7HM1L` in `Inspector section title` `QDBns`).
  InspectorSection,
  /// Geist Mono 12 `text_primary`, 1.3: a property's value that is a path or a command (`Program file` in
  /// `VIpsx`).
  PropertyMono,
  /// 13/600 `text_primary`: a form section's title over its rows (`Title` `t27V7` in `Section` `De1nE`).
  FormSection,
  /// Geist Mono 12, 1.7: environment variables (uSWRr/IBFli).
  MonoArea,
  /// 13/500, 1.2: a large button's label (`P8aXN` in `Create your first project` `rDzhV`).
  ButtonLarge,
}

impl ExtraTypography {
  pub const ALL: [Self; 19] = [
    Self::Display,
    Self::Strong,
    Self::Note,
    Self::NoteStrong,
    Self::Prose,
    Self::Lead,
    Self::ColumnHeading,
    Self::MonoKey,
    Self::MonoNote,
    Self::MonoSmall,
    Self::MonoLabel,
    Self::Initials,
    Self::OptionTitle,
    Self::Property,
    Self::InspectorSection,
    Self::PropertyMono,
    Self::FormSection,
    Self::MonoArea,
    Self::ButtonLarge,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Display => "display",
      Self::Strong => "strong",
      Self::Note => "note",
      Self::NoteStrong => "note_strong",
      Self::Prose => "prose",
      Self::Lead => "lead",
      Self::ColumnHeading => "column_heading",
      Self::MonoKey => "mono_key",
      Self::MonoNote => "mono_note",
      Self::MonoSmall => "mono_small",
      Self::MonoLabel => "mono_label",
      Self::Initials => "initials",
      Self::OptionTitle => "option_title",
      Self::Property => "property",
      Self::InspectorSection => "inspector_section",
      Self::PropertyMono => "property_mono",
      Self::FormSection => "form_section",
      Self::MonoArea => "mono_area",
      Self::ButtonLarge => "button_large",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    let sans = |size, weight, line_height, color| style(size, weight, line_height, 0.0, color);
    let mono = |size, color| mono(style(size, FontWeight::Normal, 1.4, 0.0, color));
    match self {
      Self::Display => style(28.0, FontWeight::SemiBold, 1.15, -0.5, palette.text_primary),
      Self::Strong => sans(13.0, FontWeight::Medium, 1.3, palette.text_primary),
      Self::Note => sans(11.5, FontWeight::Normal, 1.4, palette.text_muted),
      Self::NoteStrong => sans(11.5, FontWeight::Medium, 1.4, palette.text_primary),
      Self::Prose => sans(12.0, FontWeight::Normal, 1.45, palette.text_secondary),
      Self::Lead => sans(12.5, FontWeight::Normal, 1.5, palette.text_secondary),
      Self::ColumnHeading => style(10.5, FontWeight::Medium, 1.35, 0.4, palette.text_muted),
      Self::MonoKey => mono(11.5, palette.text_muted),
      Self::MonoNote => mono(11.0, palette.text_muted),
      Self::MonoSmall => mono(10.0, palette.text_secondary),
      Self::MonoLabel => self::mono(style(11.0, FontWeight::Medium, 1.35, 0.0, palette.text_secondary)),
      Self::Initials => sans(9.0, FontWeight::SemiBold, 1.0, palette.text_primary),
      Self::OptionTitle => sans(12.5, FontWeight::Medium, 1.3, palette.text_primary),
      Self::Property => sans(12.0, FontWeight::Normal, 1.3, palette.text_secondary),
      Self::InspectorSection => sans(12.0, FontWeight::SemiBold, 1.3, palette.text_primary),
      Self::PropertyMono => self::mono(sans(12.0, FontWeight::Normal, 1.3, palette.text_primary)),
      Self::FormSection => sans(13.0, FontWeight::SemiBold, 1.3, palette.text_primary),
      Self::MonoArea => self::mono(sans(12.0, FontWeight::Normal, 1.7, palette.text_primary)),
      Self::ButtonLarge => sans(13.0, FontWeight::Medium, 1.2, palette.text_primary),
    }
  }
}

/// Glyph sizes of the lucide icon font: the sizes of the design's `icon` nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
  /// 11: a locked tab's lock (`J86ch`), a removable chip's cross (`mvDTs`).
  Xs,
  /// 12: chevrons, a tab's close, status bar icons; a reason line's lock (`jww97`), a field error's
  /// mark (`Vyng0`), a checkbox's check (`zBgu0`), a compact status (`Z1oSsq`).
  Sm,
  /// 13: a tab's page icon, the project switcher's chevrons; a status (`XxCAr`), a priority
  /// (`MkM4n`), a search field's glass (`jsCS4`).
  Base,
  /// 14: the All projects tab, new tab, window controls, menu items; a button's icon (`SUTVv`), a
  /// select's chevron (`l7wBbZ`), a loading line's loader (`dCwnl`).
  Md,
  /// 15: navigation items.
  Nav,
  /// 16: the glyph of an empty or unbuilt screen; an icon button's glyph (`KKhqA`), a notice's mark
  /// (`RE5FC`).
  Lg,
  /// 18: an empty state's glyph (`WOjtr`).
  Xl,
}

impl IconSize {
  pub const ALL: [Self; 7] = [Self::Xs, Self::Sm, Self::Base, Self::Md, Self::Nav, Self::Lg, Self::Xl];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Xs => "icon_xs",
      Self::Sm => "icon_sm",
      Self::Base => "icon_base",
      Self::Md => "icon_md",
      Self::Nav => "icon_nav",
      Self::Lg => "icon_lg",
      Self::Xl => "icon_xl",
    }
  }

  pub const fn size(self) -> f32 {
    match self {
      Self::Xs => 11.0,
      Self::Sm => 12.0,
      Self::Base => 13.0,
      Self::Md => 14.0,
      Self::Nav => 15.0,
      Self::Lg => 16.0,
      Self::Xl => 18.0,
    }
  }
}

/// The type roles of the Theme board (`w3uvVL`): the dense desktop scale, body 13.
pub fn typography(palette: &ThemePalette) -> ThemeTypography {
  let primary = palette.text_primary;
  let extras = ExtraTypography::ALL
    .into_iter()
    .map(|extra| (Arc::from(extra.name()), extra.style(palette)));
  let icons = IconSize::ALL.into_iter().map(|size| {
    let glyph = TextStyle {
      font_family: Arc::from(ICONS),
      ..style(size.size(), FontWeight::Normal, 1.0, 0.0, palette.text_secondary)
    };
    (Arc::from(size.name()), glyph)
  });
  ThemeTypography {
    heading: style(20.0, FontWeight::SemiBold, 1.25, -0.2, primary),
    title: style(15.0, FontWeight::SemiBold, 1.35, -0.1, primary),
    body: style(13.0, FontWeight::Normal, 1.5, 0.0, primary),
    description: style(12.0, FontWeight::Normal, 1.5, 0.0, palette.text_secondary),
    caption: style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
    label: style(12.0, FontWeight::Medium, 1.35, 0.0, primary),
    field_label: style(11.0, FontWeight::Medium, 1.35, 0.0, palette.text_secondary),
    button: style(12.0, FontWeight::Medium, 1.2, 0.0, primary),
    link: style(
      13.0,
      FontWeight::Normal,
      1.5,
      0.0,
      palette.get(ExtraColor::AccentText.role()),
    ),
    mono: mono(style(12.0, FontWeight::Normal, 1.5, 0.0, primary)),
    extra: extras
      .chain(icons)
      .chain(super::startup::typography(palette))
      .chain(super::settings::typography(palette))
      .chain(super::accounts::typography(palette))
      .chain(super::task::typography(palette))
      .chain(super::decisions::typography(palette))
      .chain(super::shell::typography(palette))
      .chain(super::code::typography(palette))
      .chain(super::files::typography(palette))
      .chain(super::task_form::typography(palette))
      .chain(super::conversation::typography(palette))
      .chain(super::composer::typography(palette))
      .chain(super::cards::typography(palette))
      .chain(super::lifecycle::typography(palette))
      .chain(super::local_integration::typography(palette))
      .chain(super::connections::typography(palette))
      .chain(super::secrets::typography(palette))
      .chain(super::projects::typography(palette))
      .chain(super::agents::typography(palette))
      .chain(super::instructions::typography(palette))
      .chain(super::skills::typography(palette))
      .chain(super::orchestrator::typography(palette))
      .chain(super::overview::typography(palette))
      .chain(super::needs_you::typography(palette))
      .collect(),
  }
}

/// `sans` in Geist Mono without ligatures: a command, a key or a figure shows each character as typed
/// (Geist Mono would join `--` and `->`).
pub(super) fn mono(sans: TextStyle) -> TextStyle {
  TextStyle {
    font_family: Arc::from(MONO),
    font_features: FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]),
    ..sans
  }
}

pub(super) fn style(
  font_size: f32,
  weight: FontWeight,
  line_height: f32,
  letter_spacing: f32,
  color: Color,
) -> TextStyle {
  TextStyle {
    font_family: Arc::from(SANS),
    font_size,
    weight,
    line_height,
    letter_spacing,
    color,
    ..TextStyle::default()
  }
}
