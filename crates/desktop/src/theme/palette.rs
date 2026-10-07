use lurq::{
  app::theme::{PaletteColor, ThemePalette},
  node::color::Color,
};

use super::tokens::{
  ACCENT, BORDER, CANVAS, CANVAS_CLEAR, CHART_HATCH, CHART_SERIES_1, CHART_SERIES_2, CHART_SERIES_3, CODE_BG,
  CODE_COMMENT, CODE_CONSTANT, CODE_FUNCTION, CODE_KEYWORD, CODE_STRING, CODE_TEXT, CODE_VARIABLE, DANGER, DIFF_ADD_BG,
  DIFF_ADD_TEXT, DIFF_REMOVE_BG, DIFF_REMOVE_TEXT, FOCUS_GAP, FOCUS_RING, HAIRLINE, HOVER, INVERSE_TEXT, MUTED, RAISED,
  SCRIM, SIDEBAR, SUBTLE, SURFACE, TEXT, TRAFFIC_CLOSE, TRAFFIC_MINIMIZE, TRAFFIC_ZOOM,
};

/// Pencil colors without a built-in lurq role, registered as `PaletteColor::Extra`.
/// Use these instead of spelling extra names at call sites.
#[derive(Clone, Copy)]
pub enum ExtraColor {
  /// Cards and panels on the canvas (Pencil `surface`, e.g. `iN90G`).
  Surface,
  /// Row and section dividers (Pencil `hairline`, e.g. `zemhy`).
  Hairline,
  /// Dims the window behind a dialog (`a8U756`).
  Scrim,
  /// The light ring of a focused control (Pencil `focus-ring`, `xURvJ`).
  FocusRing,
  /// The dark gap between a focused control and its ring (Pencil `focus-gap`, `xURvJ`).
  FocusGap,
  /// Unchanged diff lines (Pencil `code-bg`, `vEZ42`).
  CodeBackground,
  /// Code without a more specific role, and punctuation (Pencil `code-text`).
  CodeText,
  /// Keywords, storage and operators (Pencil `code-keyword`).
  CodeKeyword,
  /// Strings (Pencil `code-string`).
  CodeString,
  /// Function names (Pencil `code-function`).
  CodeFunction,
  /// Declared names: types, variables (Pencil `code-variable`).
  CodeVariable,
  /// Numbers, constants and built-ins (Pencil `code-constant`).
  CodeConstant,
  /// Comments, and the line numbers of a diff (Pencil `code-comment`).
  CodeComment,
  /// Added diff lines (Pencil `diff-add-bg`).
  DiffAddedBackground,
  /// The `+` of an added line (Pencil `diff-add-text`).
  DiffAddedMark,
  /// Removed diff lines (Pencil `diff-remove-bg`).
  DiffRemovedBackground,
  /// The `−` of a removed line (Pencil `diff-remove-text`).
  DiffRemovedMark,
  /// Transparent `canvas`, where the tab strip's edge fade ends (`xpSC0`).
  CanvasClear,
  /// The macOS close, minimize and zoom buttons (`R3RtGs` in `V70DN`).
  TrafficClose,
  TrafficMinimize,
  TrafficZoom,
  /// The Usage dashboard's chart series: Codex, Claude Code and a third CLI (`chart-series-1/2/3`).
  ChartSeries1,
  ChartSeries2,
  ChartSeries3,
  /// The hatching before a CLI's first plan limits report.
  ChartHatch,
}

impl ExtraColor {
  const ALL: [Self; 25] = [
    Self::Surface,
    Self::Hairline,
    Self::Scrim,
    Self::FocusRing,
    Self::FocusGap,
    Self::CodeBackground,
    Self::CodeText,
    Self::CodeKeyword,
    Self::CodeString,
    Self::CodeFunction,
    Self::CodeVariable,
    Self::CodeConstant,
    Self::CodeComment,
    Self::DiffAddedBackground,
    Self::DiffAddedMark,
    Self::DiffRemovedBackground,
    Self::DiffRemovedMark,
    Self::CanvasClear,
    Self::TrafficClose,
    Self::TrafficMinimize,
    Self::TrafficZoom,
    Self::ChartSeries1,
    Self::ChartSeries2,
    Self::ChartSeries3,
    Self::ChartHatch,
  ];

  fn name(self) -> &'static str {
    match self {
      Self::Surface => "surface",
      Self::Hairline => "hairline",
      Self::Scrim => "scrim",
      Self::FocusRing => "focus-ring",
      Self::FocusGap => "focus-gap",
      Self::CodeBackground => "code-bg",
      Self::CodeText => "code-text",
      Self::CodeKeyword => "code-keyword",
      Self::CodeString => "code-string",
      Self::CodeFunction => "code-function",
      Self::CodeVariable => "code-variable",
      Self::CodeConstant => "code-constant",
      Self::CodeComment => "code-comment",
      Self::DiffAddedBackground => "diff-add-bg",
      Self::DiffAddedMark => "diff-add-text",
      Self::DiffRemovedBackground => "diff-remove-bg",
      Self::DiffRemovedMark => "diff-remove-text",
      Self::CanvasClear => "canvas-clear",
      Self::TrafficClose => "traffic-close",
      Self::TrafficMinimize => "traffic-minimize",
      Self::TrafficZoom => "traffic-zoom",
      Self::ChartSeries1 => "chart-series-1",
      Self::ChartSeries2 => "chart-series-2",
      Self::ChartSeries3 => "chart-series-3",
      Self::ChartHatch => "chart-hatch",
    }
  }

  fn value(self) -> Color {
    Color::from_hex(match self {
      Self::Surface => SURFACE,
      Self::Hairline => HAIRLINE,
      Self::Scrim => SCRIM,
      Self::FocusRing => FOCUS_RING,
      Self::FocusGap => FOCUS_GAP,
      Self::CodeBackground => CODE_BG,
      Self::CodeText => CODE_TEXT,
      Self::CodeKeyword => CODE_KEYWORD,
      Self::CodeString => CODE_STRING,
      Self::CodeFunction => CODE_FUNCTION,
      Self::CodeVariable => CODE_VARIABLE,
      Self::CodeConstant => CODE_CONSTANT,
      Self::CodeComment => CODE_COMMENT,
      Self::DiffAddedBackground => DIFF_ADD_BG,
      Self::DiffAddedMark => DIFF_ADD_TEXT,
      Self::DiffRemovedBackground => DIFF_REMOVE_BG,
      Self::DiffRemovedMark => DIFF_REMOVE_TEXT,
      Self::CanvasClear => CANVAS_CLEAR,
      Self::TrafficClose => TRAFFIC_CLOSE,
      Self::TrafficMinimize => TRAFFIC_MINIMIZE,
      Self::TrafficZoom => TRAFFIC_ZOOM,
      Self::ChartSeries1 => CHART_SERIES_1,
      Self::ChartSeries2 => CHART_SERIES_2,
      Self::ChartSeries3 => CHART_SERIES_3,
      Self::ChartHatch => CHART_HATCH,
    })
  }

  pub fn role(self) -> PaletteColor {
    PaletteColor::extra(self.name())
  }
}

pub fn palette() -> ThemePalette {
  let c = Color::from_hex;
  let mut palette = ThemePalette {
    accent: c(ACCENT),
    // Not specified in the design: hover brightens the primary action towards `text`.
    accent_hover: c(TEXT),
    accent_muted: c(HOVER),
    surface_base: c(CANVAS),
    surface_panel: c(SIDEBAR),
    surface_raised: c(RAISED),
    // Fields are filled with `sidebar` in the design (`ZGwlv`).
    surface_input: c(SIDEBAR),
    border: c(BORDER),
    // Not specified in the design; the strongest neutral below `muted`.
    border_strong: c(SUBTLE),
    // Focused fields use a `text` stroke (`o8Oug`).
    border_focus: c(TEXT),
    text_primary: c(TEXT),
    text_secondary: c(MUTED),
    text_muted: c(SUBTLE),
    text_inverse: c(INVERSE_TEXT),
    // Only the Unreadable status label is in colour (`mGbYs`); status otherwise pairs an icon with a
    // label, so the success/warning/info roles and danger's muted fill keep lurq defaults.
    danger: c(DANGER),
    ..ThemePalette::default()
  };
  for extra in ExtraColor::ALL {
    palette.extra.insert(extra.name().into(), extra.value());
  }
  palette
}
