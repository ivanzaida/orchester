//! The hint over the page while the system's file dialog is open for a file field of the person's tab (`03b · File
//! chooser · dialog open` `TWpyp`): the page area dimmed, and in its middle what to do (*Choose a file in the dialog
//! that opened*) and which page asks for which field. Orchester adds no modal of its own and no buttons: the dialog is
//! the system's (`browser::files`), and the hint goes once it closes. The line holds the field's name, the page's
//! text, so it is sensitive: painted, but redacted in the MCP and DevTools trees and their screenshots.

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::BrowserTabId;

use crate::{
  browser::{model, state::Showing},
  theme::{BrowserSpacing, BrowserText, ExtraColor, ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// The hint over the page while the request of `tab` for files waits for the system's dialog.
pub fn hint(ctx: &Ctx, showing: &Showing, tab: &BrowserTabId) -> Option<Element> {
  let chooser = showing.view.as_ref()?.file_chooser(tab)?;
  let origin = showing
    .strip
    .find(tab)
    .map(|shown| model::host(&shown.url))
    .unwrap_or_default();
  let line = match &chooser.label {
    Some(label) => ctx.t_args(
      "browser.fileChooser.asksFor",
      [("origin", origin.as_str()), ("label", label.as_str())],
    ),
    None => ctx.t_args("browser.fileChooser.asks", [("origin", origin.as_str())]),
  };
  let words = Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&ctx.t("browser.fileChooser.title"))
        .id("browser-file-hint-title")
        .variant(BrowserText::FileHintTitle.role()),
    )
    .child(
      Text::new(&line)
        .sensitive()
        .id("browser-file-hint-line")
        .variant(BrowserText::FileHintLine.role()),
    );
  let card = Row::new()
    .id("browser-file-hint")
    .padding(
      Padding::new()
        .top(ExtraSpacing::Inset.role())
        .bottom(ExtraSpacing::Inset.role())
        .left(BrowserSpacing::FileHintSides.role())
        .right(BrowserSpacing::FileHintSides.role()),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(icon(Icon::FolderOpen, IconSize::Lg, PaletteColor::TextSecondary))
    .child(words);
  Some(
    Column::new()
      .id("browser-file-chooser")
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .background(ExtraColor::Scrim.role())
      .justify(Justify::Center)
      .align_items(Alignment::Center)
      // The page under the scrim takes no input while the dialog is open.
      .on_mouse_down(|event: MouseEvent| event.stop_propagation())
      .on_mouse_up(|event: MouseEvent| event.stop_propagation())
      .child(card)
      .into(),
  )
}
