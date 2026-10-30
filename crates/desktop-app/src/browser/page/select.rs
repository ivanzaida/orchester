//! A `<select>` list the person opened on the tab shown (`04 · Select list` `r31HX`), which a headless frame does not
//! show: drawn under its field, as wide as it, in the menu's look (`surface_raised` in a `border_strong` stroke, radius
//! 8, rows of 28 with the chosen one checked). Choosing an option sets it in the page; a press outside the list or a
//! choice closes it. The options are the page's text, so they are sensitive: painted, but redacted in the MCP and
//! DevTools trees and their screenshots.

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Stack, Text, TextOverflow},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::{BrowserCommand, BrowserTabId};

use super::Scale;
use crate::{
  browser::state::Showing,
  services::Services,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The list under the `<select>` of `tab` the person opened, while it is open.
pub fn list(ctx: &mut Ctx, showing: &Showing, tab: &BrowserTabId, scale: Scale) -> Option<Element> {
  let select = showing
    .view
    .as_ref()?
    .select()
    .filter(|select| &select.tab == tab)?
    .clone();
  let browser = Services::of(ctx).runners.browser_window();
  let root = showing.project.as_ref().clone();
  let element = select.list.element;
  let left = element.x as f32 * scale.x;
  let top = (element.y as f32 + element.height as f32) * scale.y + ExtraSpacing::Snug.value();
  let width = element.width as f32 * scale.x;
  let mut rows = Column::new()
    .id("browser-select-list")
    .absolute(left, top, Dimension::Px(width), Dimension::Auto)
    .padding(SpacingSize::Xs)
    .spacing(ExtraSpacing::Tight.role())
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Md)
    .on_mouse_down(|event: MouseEvent| event.stop_propagation());
  for (index, option) in (0_u32..).zip(&select.list.options) {
    let (commands, browser, root, tab) = (showing.commands.clone(), browser.clone(), root.clone(), tab.clone());
    let id = select.list.select;
    let row = pressable(move || {
      browser.close_select(&root);
      commands.send(BrowserCommand::ChooseOption {
        tab: tab.clone(),
        select: id,
        index,
      });
    })
    .id(format!("browser-select-option-{index}"))
    .describe("chosen", if option.selected { "true" } else { "false" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(
      Text::new(&option.label)
        .sensitive()
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(
      option
        .selected
        .then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)),
    );
    rows = rows.child(if option.disabled {
      row
        .opacity(DISABLED_OPACITY)
        .hit_test(lurq::node::HitTestBehavior::None)
        .into()
    } else {
      Element::from(row)
    });
  }
  // A stack, as lurq places absolute children only there.
  Some(
    Stack::new()
      .id("browser-select")
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .on_mouse_down(move |event: MouseEvent| {
        event.stop_propagation();
        browser.close_select(&root);
      })
      .child(rows)
      .into(),
  )
}
