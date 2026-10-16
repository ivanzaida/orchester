//! How a link in a Markdown of the window looks, an Orchestrator's reply first (`Orchestrator · links · states`
//! `agv8r`, `Orchestrator · reply with links` `w262B`), in the terms of lurq's Markdown link presentation. A link the
//! chat follows: its words in the theme's link role over a 1 `text_muted` underline; under the pointer the words and
//! underline in `accent_hover` on the `hover` fill, square at the underline, which ends where the fill does. A link to
//! the web is followed by its host in 11.5 `text_muted` and an 11 `arrow-up-right`, 6 after the words and 3 apart,
//! `text_secondary` under the pointer. A link the chat does not follow is followed by its address in Geist Mono 11.5
//! `text_muted`. The focus ring and the tooltip are every Markdown link's (`markdown`).

use lurq::{
  app::theme::{MarkdownInlineStyle, MarkdownLinkState, Theme},
  layout::text_style::{TextStyle, TextUnderline},
  node::{border::BorderRadius, color::Color},
};

use super::{IconSize, OrchestratorText, markdown::LINK_RADIUS, palette::ExtraColor};

/// A space's advance in Geist, of its font size: the gaps after a link are spaces widened to the design's.
const SPACE_EM: f32 = 0.25;
/// From a link's words to its host, and from the host to its arrow (`ZzJCm` in `agv8r`).
const HOST_GAP: f32 = 6.0;
const ICON_GAP: f32 = 3.0;
/// The underline's thickness (`c6Tcl` in `agv8r`).
const UNDERLINE: f32 = 1.0;
/// Geist's ascent and descent, of its font size (`hhea` of the bundled `Geist-*.ttf`): where a line's baseline sits
/// in its line box.
const GEIST_ASCENT: f32 = 1.005;
const GEIST_DESCENT: f32 = 0.295;

/// The looks of every Markdown link of the window, read from the theme's roles.
#[derive(Clone, PartialEq)]
pub struct ChatLinkLooks {
  /// The words of a link the chat follows.
  pub link: MarkdownInlineStyle,
  pub hovered: MarkdownLinkState,
  /// With keyboard focus the ring stands for the underline, which goes (`Mjhop` in `agv8r`).
  pub focused: MarkdownLinkState,
  /// The hovered fill's corners.
  pub radius: BorderRadius,
  pub host: MarkdownInlineStyle,
  /// A space as wide as the gap from the words to the host.
  pub host_gap: MarkdownInlineStyle,
  /// The arrow after the host, a glyph of the icon font.
  pub icon: MarkdownInlineStyle,
  /// A space as wide as the gap from the host to the arrow.
  pub icon_gap: MarkdownInlineStyle,
  pub address: MarkdownInlineStyle,
}

impl ChatLinkLooks {
  /// The looks of links in text set in `text`.
  pub fn of(theme: &Theme, text: &TextStyle) -> Self {
    let palette = theme.palette();
    let mut link = theme.markdown().link.clone();
    link.text.underline = Some(
      TextUnderline::new()
        .color(palette.text_muted)
        .thickness(UNDERLINE)
        .offset(underline_offset(text)),
    );
    let hovered = MarkdownLinkState::new()
      .color(palette.accent_hover)
      .underline_color(palette.accent_hover)
      .background(palette.get(ExtraColor::Hover.role()))
      .trailing_color(palette.text_secondary);
    let host = theme.typography_style(OrchestratorText::LinkHost.role());
    let mut icon = inline(&theme.typography_style(IconSize::Xs.role()));
    icon.text.color = Some(palette.text_muted);
    Self {
      link,
      hovered,
      focused: MarkdownLinkState::new().underline_color(Color::new(0, 0, 0, 0)),
      radius: BorderRadius::new(LINK_RADIUS, LINK_RADIUS, 0.0, 0.0),
      host: inline(&host),
      host_gap: gap(&host, HOST_GAP),
      icon,
      icon_gap: gap(&host, ICON_GAP),
      address: inline(&theme.typography_style(OrchestratorText::LinkAddress.role())),
    }
  }
}

/// How far below the baseline the underline's top sits so that it ends where the line box does, where a hovered
/// link's fill ends (`K52XFs` in `agv8r`): the half leading and the descent under the baseline, less the line.
fn underline_offset(text: &TextStyle) -> f32 {
  let leading = text.line_height - (GEIST_ASCENT + GEIST_DESCENT);
  (leading / 2.0 + GEIST_DESCENT) * text.font_size - UNDERLINE
}

/// `style` as a run's style over the text around it.
fn inline(style: &TextStyle) -> MarkdownInlineStyle {
  let mut inline = MarkdownInlineStyle::new();
  inline.text.font_family = Some(style.font_family.clone());
  inline.text.font_size = Some(style.font_size);
  inline.text.weight = Some(style.weight);
  inline.text.font_features = Some(style.font_features.clone());
  inline.text.color = Some(style.color);
  inline
}

/// A space in `style` widened to `width`.
fn gap(style: &TextStyle, width: f32) -> MarkdownInlineStyle {
  let mut gap = inline(style);
  gap.text.letter_spacing = Some(width - style.font_size * SPACE_EM);
  gap
}
