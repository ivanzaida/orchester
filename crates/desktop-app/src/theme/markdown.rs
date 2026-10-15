//! How Markdown an agent or a person wrote reads (`ui::markdown`). The design draws only its plain
//! parts: body text in the caller's style (a chat message 13/1.5 `D8JGX4`, a plan 12.5/1.6 `vGWCj`), a
//! heading at the body's size in 600 `text_primary` (`iQEpj` in `Q5J0r`), links in `accent_text` 500
//! (`p86ea`), code in Geist Mono on `code_bg` inside a `border` stroke, radius 6, as the command box
//! (`yhiM8`). Everything else comes from existing variables: larger headings step up from the body,
//! quotes sit on `surface_raised`, list markers and tables use the muted text and borders. The colors are
//! the palette's, so the theme sets them again when the mode changes.

use std::sync::Arc;

use lurq::{
  app::theme::{MarkdownBlockStyle, MarkdownInlineStyle, ThemeMarkdown, ThemePalette},
  components::Text,
  layout::text_style::{FontFeature, FontFeatures, FontWeight, TextStyle},
  markdown::MarkdownHeadingLevel,
  node::color::Color,
};

use super::{ExtraColor, fonts::MONO};

const CODE_RADIUS: f32 = 6.0;
const CODE_PADDING: f32 = 8.0;
const BLOCK_SPACING: f32 = 8.0;
const ITEM_SPACING: f32 = 2.0;
/// The room before a list item's text: a numeral with its period, a bullet.
const ORDERED_MARKER: f32 = 20.0;
const BULLET_MARKER: f32 = 12.0;

fn inline(configure: impl FnOnce(&mut MarkdownInlineStyle)) -> MarkdownInlineStyle {
  let mut style = MarkdownInlineStyle::new();
  configure(&mut style);
  style
}

fn heading(scale: f32, color: Color) -> MarkdownInlineStyle {
  inline(|style| {
    style.text.font_size_scale = Some(scale);
    style.text.weight = Some(FontWeight::SemiBold);
    style.text.color = Some(color);
  })
}

/// Geist Mono without ligatures, a little smaller than the text around it.
fn code(line_height: Option<f32>, color: Color) -> MarkdownInlineStyle {
  inline(|style| {
    style.text.font_family = Some(Arc::from(MONO));
    style.text.font_size_scale = Some(0.92);
    style.text.weight = Some(FontWeight::Normal);
    style.text.font_features = Some(FontFeatures::from([
      FontFeature::disable(*b"liga"),
      FontFeature::disable(*b"calt"),
    ]));
    style.text.color = Some(color);
    style.text.line_height = line_height;
  })
}

fn colored(color: Color) -> MarkdownInlineStyle {
  inline(|style| style.text.color = Some(color))
}

fn boxed(background: Option<Color>, border: Color, padding: Option<f32>) -> MarkdownBlockStyle {
  MarkdownBlockStyle {
    background,
    border_color: Some(border),
    border_width: Some(1.0),
    radius: Some(CODE_RADIUS),
    padding,
  }
}

pub fn markdown(palette: &ThemePalette) -> ThemeMarkdown {
  let link = inline(|style| {
    style.text.weight = Some(FontWeight::Medium);
    style.text.color = Some(palette.get(ExtraColor::AccentText.role()));
  });
  let strong = inline(|style| {
    style.text.weight = Some(FontWeight::SemiBold);
    style.text.color = Some(palette.text_primary);
  });
  let sans = MarkdownInlineStyle::new();
  ThemeMarkdown {
    document_spacing: BLOCK_SPACING,
    list_item_spacing: ITEM_SPACING,
    nested_block_spacing: ITEM_SPACING,
    code_block_spacing: BLOCK_SPACING,
    blockquote_gap: BLOCK_SPACING,
    list_marker_gap: ITEM_SPACING,
    ordered_list_marker_width: ORDERED_MARKER,
    unordered_list_marker_width: BULLET_MARKER,
    heading: heading(1.0, palette.text_primary),
    heading_1: heading(1.15, palette.text_primary),
    heading_2: heading(1.08, palette.text_primary),
    heading_3: heading(1.0, palette.text_primary),
    heading_4: heading(1.0, palette.text_primary),
    heading_5: heading(1.0, palette.text_primary),
    heading_6: heading(1.0, palette.text_primary),
    strong,
    link,
    inline_code: code(None, palette.text_primary),
    code_block: code(Some(1.5), palette.text_primary),
    code_block_label: colored(palette.text_muted),
    list_marker: colored(palette.text_muted),
    blockquote_marker: colored(palette.text_muted),
    table_header: colored(palette.text_primary),
    table_cell: sans,
    table_marker: colored(palette.text_muted),
    code_block_box: boxed(
      Some(palette.get(ExtraColor::CodeBackground.role())),
      palette.border,
      Some(CODE_PADDING),
    ),
    blockquote_box: boxed(Some(palette.surface_raised), palette.border, Some(CODE_PADDING)),
    table_box: boxed(None, palette.border, None),
    table_header_box: MarkdownBlockStyle {
      padding: Some(CODE_PADDING),
      ..MarkdownBlockStyle::new()
    },
    table_cell_box: MarkdownBlockStyle {
      padding: Some(CODE_PADDING),
      ..MarkdownBlockStyle::new()
    },
    // A table's columns share the text's width, however narrow: it never runs past its text.
    table_cell_min_width: 0.0,
    ..ThemeMarkdown::default()
  }
}

/// A run of Markdown text in `style`, which a caller resolved from this theme's Markdown roles over a text role:
/// for a reply the chat draws itself where lurq's Markdown draws every link alike (`screens::orchestrator::reply`).
pub fn run(text: &str, style: TextStyle) -> Text {
  Text::styled(text, style)
}

/// A heading's text style over `base`, the body's, as lurq's Markdown sets it: for a reply the chat draws itself
/// (`screens::orchestrator::reply`).
pub fn heading_style(theme: &ThemeMarkdown, level: MarkdownHeadingLevel, base: &TextStyle) -> TextStyle {
  let shared = theme.heading.apply(base);
  match level {
    MarkdownHeadingLevel::H1 => theme.heading_1.apply(&shared),
    MarkdownHeadingLevel::H2 => theme.heading_2.apply(&shared),
    MarkdownHeadingLevel::H3 => theme.heading_3.apply(&shared),
    MarkdownHeadingLevel::H4 => theme.heading_4.apply(&shared),
    MarkdownHeadingLevel::H5 => theme.heading_5.apply(&shared),
    MarkdownHeadingLevel::H6 => theme.heading_6.apply(&shared),
  }
}
