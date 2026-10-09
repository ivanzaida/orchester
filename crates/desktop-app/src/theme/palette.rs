use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, ThemePalette},
  node::color::Color,
};

/// The `mode` axis of the design's themes. Orchester uses the `graphite` value of the `accent` axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
  #[default]
  Dark,
  Light,
}

impl Mode {
  /// The name of the mode in the design and in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Dark => "dark",
      Self::Light => "light",
    }
  }

  fn pick(self, dark: &str, light: &str) -> Color {
    Color::from_hex(match self {
      Self::Dark => dark,
      Self::Light => light,
    })
  }
}

/// Design colors without a built-in lurq role, registered as `PaletteColor::Extra`. Call sites use
/// these instead of spelling extra names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraColor {
  /// `accent_text`: links and accent-colored text.
  AccentText,
  /// `on_accent`: text and icons on an `accent` fill.
  OnAccent,
  /// `border_panel`: the outline of the shell's content panel (`Content · Overview` in `q9mf0k`).
  PanelBorder,
  /// `hover`: overlay on hovered rows and buttons; the selected navigation item and active tab.
  Hover,
  /// `pressed`: overlay on pressed rows and buttons.
  Pressed,
  /// `selection`: selected text and selection outlines.
  Selection,
  /// `scrim`: dims the screen behind a dialog.
  Scrim,
  /// `shadow`: the color of the elevation shadows.
  Shadow,
  /// `own_message`: the person's own message in a conversation.
  OwnMessage,
  /// `code_bg`: the background of code and diffs.
  CodeBackground,
  /// `code_keyword`: keywords in code and diffs.
  CodeKeyword,
  /// `code_string`: string literals in code and diffs.
  CodeString,
  /// `code_number`: number literals in code and diffs.
  CodeNumber,
  /// `diff_add`: an added line of a diff.
  DiffAdded,
  /// `diff_remove`: a removed line of a diff.
  DiffRemoved,
  /// The Orchester mark's tile (`Brand / Orchester` `OLHng`), the same in both modes.
  BrandTile,
  /// The Orchester mark's bars, the same in both modes.
  BrandMark,
}

impl ExtraColor {
  pub const ALL: [Self; 17] = [
    Self::AccentText,
    Self::OnAccent,
    Self::PanelBorder,
    Self::Hover,
    Self::Pressed,
    Self::Selection,
    Self::Scrim,
    Self::Shadow,
    Self::OwnMessage,
    Self::CodeBackground,
    Self::CodeKeyword,
    Self::CodeString,
    Self::CodeNumber,
    Self::DiffAdded,
    Self::DiffRemoved,
    Self::BrandTile,
    Self::BrandMark,
  ];

  pub fn role(self) -> PaletteColor {
    PaletteColor::extra(self.name())
  }

  /// The design variable's name.
  pub const fn name(self) -> &'static str {
    match self {
      Self::AccentText => "accent_text",
      Self::OnAccent => "on_accent",
      Self::PanelBorder => "border_panel",
      Self::Hover => "hover",
      Self::Pressed => "pressed",
      Self::Selection => "selection",
      Self::Scrim => "scrim",
      Self::Shadow => "shadow",
      Self::OwnMessage => "own_message",
      Self::CodeBackground => "code_bg",
      Self::CodeKeyword => "code_keyword",
      Self::CodeString => "code_string",
      Self::CodeNumber => "code_number",
      Self::DiffAdded => "diff_add",
      Self::DiffRemoved => "diff_remove",
      Self::BrandTile => "brand_tile",
      Self::BrandMark => "brand_mark",
    }
  }

  fn color(self, mode: Mode) -> Color {
    match self {
      Self::AccentText => mode.pick("#EDEDF0", "#18181B"),
      Self::OnAccent => mode.pick("#111113", "#FFFFFF"),
      Self::PanelBorder => mode.pick("#2A2A2E", "#C7C7CE"),
      Self::Hover => mode.pick("#FFFFFF0F", "#0000000A"),
      Self::Pressed => mode.pick("#FFFFFF17", "#00000012"),
      Self::Selection => mode.pick("#2F6FEB", "#2F6FEB"),
      Self::Scrim => mode.pick("#000000A3", "#18181B59"),
      Self::Shadow => mode.pick("#00000099", "#0F0F1A1F"),
      Self::OwnMessage => mode.pick("#232326", "#F1F1F4"),
      Self::CodeBackground => mode.pick("#0E0E10", "#F6F6F8"),
      Self::CodeKeyword => mode.pick("#8AB4F8", "#2B5FC7"),
      Self::CodeString => mode.pick("#7DCFBF", "#0F7A6A"),
      Self::CodeNumber => mode.pick("#C3A6F7", "#7A4CC2"),
      Self::DiffAdded => mode.pick("#3DB07A1F", "#15835414"),
      Self::DiffRemoved => mode.pick("#F0605A1F", "#D4342F14"),
      Self::BrandTile => mode.pick("#18191D", "#18191D"),
      Self::BrandMark => mode.pick("#F1F1F4", "#F1F1F4"),
    }
  }
}

/// The color variables of `orchester2.pen` for `mode` and the `graphite` accent.
pub fn palette(mode: Mode) -> ThemePalette {
  let c = |dark, light| mode.pick(dark, light);
  ThemePalette {
    accent: c("#EDEDF0", "#18181B"),
    accent_hover: c("#FFFFFF", "#2E2E33"),
    accent_muted: c("#FFFFFF14", "#18181B0F"),
    surface_base: c("#111113", "#E4E4E8"),
    surface_panel: c("#18181A", "#FFFFFF"),
    surface_raised: c("#202023", "#FFFFFF"),
    surface_input: c("#1B1B1E", "#FFFFFF"),
    border: c("#2A2A2E", "#E2E2E6"),
    border_strong: c("#3B3B40", "#C7C7CE"),
    border_focus: c("#A3A3AB", "#52525A"),
    text_primary: c("#EDEDF0", "#18181B"),
    text_secondary: c("#B3B3BA", "#45454D"),
    text_muted: c("#8A8A92", "#6B6B74"),
    text_inverse: c("#111113", "#FFFFFF"),
    success: c("#3DB07A", "#158354"),
    success_muted: c("#3DB07A24", "#1583541A"),
    warning: c("#E2A33C", "#9E630E"),
    warning_muted: c("#E2A33C24", "#D9911E24"),
    danger: c("#F0605A", "#D4342F"),
    danger_muted: c("#F0605A24", "#D4342F1A"),
    info: c("#4DA3F5", "#1A73C9"),
    info_muted: c("#4DA3F524", "#1A73C91A"),
    extra: ExtraColor::ALL
      .into_iter()
      .map(|extra| (Arc::from(extra.name()), extra.color(mode)))
      .collect(),
  }
}
