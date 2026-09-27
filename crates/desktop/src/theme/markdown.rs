//! How agent-written Markdown reads (a plan under review, 02.11 `x3lVC`), in the design's
//! grayscale: the design draws no Markdown, so every value comes from an existing variable. The
//! body is the caller's text style (agent text, 12/1.6, `FP6BD`); headings stay within the plan's
//! type scale below its 19px title (`J7irj`): 14, 13 and 12, medium; code keeps the code colors of
//! the diff (`vEZ42`); rules and quotes use the chrome's borders.

use std::sync::Arc;

use lurq::{
  app::theme::{MarkdownBlockStyle, MarkdownInlineStyle, ThemeMarkdown},
  layout::text_style::{FontFeature, FontFeatures, FontWeight},
  node::color::Color,
};

use super::tokens::{
  BORDER, CODE_BG, CODE_TEXT, HAIRLINE, MONO, MUTED, RADIUS_CONTROL, RAISED, SPACE_SM, SPACE_STACK, SPACE_TIGHT,
  SUBTLE, TEXT,
};

fn inline(configure: impl FnOnce(&mut MarkdownInlineStyle)) -> MarkdownInlineStyle {
  let mut style = MarkdownInlineStyle::new();
  configure(&mut style);
  style
}

fn heading(scale: f32) -> MarkdownInlineStyle {
  inline(|style| {
    style.text.font_size_scale = Some(scale);
    style.text.weight = Some(FontWeight::Medium);
    style.text.color = Some(Color::from_hex(TEXT));
  })
}

/// Geist Mono without ligatures, as mono text reads everywhere else (see `typography::mono`).
fn code(line_height: Option<f32>) -> MarkdownInlineStyle {
  inline(|style| {
    style.text.font_family = Some(Arc::from(MONO));
    style.text.font_size_scale = Some(0.92);
    style.text.font_features = Some(FontFeatures::from([
      FontFeature::disable(*b"liga"),
      FontFeature::disable(*b"calt"),
    ]));
    style.text.color = Some(Color::from_hex(CODE_TEXT));
    style.text.line_height = line_height;
  })
}

fn marker(color: &str) -> MarkdownInlineStyle {
  inline(|style| style.text.color = Some(Color::from_hex(color)))
}

fn block(background: Option<&str>, border: &str, border_width: f32, padding: Option<f32>) -> MarkdownBlockStyle {
  MarkdownBlockStyle {
    background: background.map(Color::from_hex),
    border_color: Some(Color::from_hex(border)),
    border_width: Some(border_width),
    radius: Some(RADIUS_CONTROL),
    padding,
  }
}

pub fn markdown() -> ThemeMarkdown {
  let strong = inline(|style| {
    style.text.weight = Some(FontWeight::Medium);
    style.text.color = Some(Color::from_hex(TEXT));
  });
  ThemeMarkdown {
    document_spacing: SPACE_STACK,
    list_item_spacing: SPACE_TIGHT,
    nested_block_spacing: SPACE_TIGHT,
    code_block_spacing: SPACE_SM,
    blockquote_gap: SPACE_STACK,
    list_marker_gap: SPACE_SM,
    heading: heading(1.0),
    heading_1: heading(14.0 / 12.0),
    heading_2: heading(13.0 / 12.0),
    heading_3: heading(1.0),
    heading_4: heading(1.0),
    heading_5: heading(1.0),
    heading_6: heading(1.0),
    strong: strong.clone(),
    link: strong,
    inline_code: code(None),
    code_block: code(Some(1.6)),
    code_block_label: marker(SUBTLE),
    list_marker: marker(MUTED),
    blockquote_marker: marker(SUBTLE),
    table_header: inline(|style| style.text.weight = Some(FontWeight::Medium)),
    table_cell: MarkdownInlineStyle::new(),
    table_marker: marker(SUBTLE),
    code_block_box: block(Some(CODE_BG), BORDER, 1.0, Some(SPACE_STACK)),
    blockquote_box: block(Some(RAISED), HAIRLINE, 2.0, Some(SPACE_STACK)),
    table_box: block(None, BORDER, 1.0, None),
    table_header_box: MarkdownBlockStyle {
      background: Some(Color::from_hex(RAISED)),
      padding: Some(SPACE_SM),
      ..MarkdownBlockStyle::new()
    },
    table_cell_box: MarkdownBlockStyle {
      padding: Some(SPACE_SM),
      ..MarkdownBlockStyle::new()
    },
    ..ThemeMarkdown::default()
  }
}
