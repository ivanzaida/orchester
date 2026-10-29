//! One tab of the Browser window's strip (`Tab · …` in `X1V8q`, indicators `zxkfr`): the page's icon (a globe for an
//! agent's tab, the person icon for the person's own), with its dot, and its label: *ORC-107 - orders-sync · Kontur
//! Admin* for an agent's tab, the page title for the person's. The label is one line cut with … by measured width, the
//! title first since the key leads, with the whole label in a tooltip only while it is cut. The tab shown is filled
//! and has its ×; another shows its × under the pointer. A click shows the tab; a middle click or × closes it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{MouseButton, MouseEvent},
    theme::{PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Button, Row, Stack},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_ipc::browser::BrowserCommand;

use super::{
  model::{Indicator, StripTab},
  state::Showing,
};
use crate::{
  theme::{BrowserBorder, ExtraColor, ExtraRadius, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{in_title_bar, pressable},
    tooltip::Truncated,
  },
};

/// A tab's height (`Tab · …` in `X1V8q`), its close button and its dot (`Indicator · new` `XZLhY`), which sits over the
/// icon's top right corner, 7 from its left and 2 over its top.
pub const TAB_HEIGHT: f32 = 26.0;
const CLOSE_SIDE: f32 = 18.0;
const DOT: f32 = 7.0;
const DOT_LEFT: f32 = 7.0;
const DOT_RAISE: f32 = 2.0;

/// The tab's props: the tab, its place, its width and the project shown.
#[derive(Clone, PartialEq)]
pub struct TabProps {
  pub tab: StripTab,
  pub index: usize,
  pub width: f32,
  pub shown: bool,
  pub showing: Showing,
}

impl DevtoolsInspectable for TabProps {}

pub struct Tab {
  hovered: Signal<bool>,
}

impl Component for Tab {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let TabProps {
      tab,
      index,
      width,
      shown,
      showing,
    } = ctx.props::<TabProps>().clone();
    let id = format!("browser-tab-{index}");
    let label = label(ctx, &tab);
    let hovered = self.hovered.get();
    let (pick, hooks) = (tab.id.clone(), showing.person.clone());
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let closing = (showing.commands, tab.id.clone());
    let middle = closing.clone();
    let button = in_title_bar(pressable(move || hooks.pick(&pick)))
      .id(id.clone())
      .describe("a11y_name", label.clone())
      .describe("state", if shown { "shown" } else { "hidden" })
      .describe("owner", if tab.is_person() { "person" } else { "run" })
      .describe("indicator", tab.indicator.map_or("none", Indicator::name))
      .width(Dimension::Px(width))
      .height(Dimension::Px(TAB_HEIGHT))
      .flex_shrink(0.0)
      .rounded(RadiusSize::Md)
      .spacing(ExtraSpacing::Snug.role())
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_mouse_up(move |event: MouseEvent| {
        if event.button == MouseButton::Middle {
          event.prevent_default();
          middle.0.send(BrowserCommand::CloseTab { tab: middle.1.clone() });
        }
      });
    let with_close = shown || hovered;
    let padding = Padding::horizontal(ExtraSpacing::Loose.role());
    let button = button.padding(if with_close {
      padding.right(ExtraSpacing::Snug.role())
    } else {
      padding
    });
    let button = if shown || hovered {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    let (glyph, text, style) = match (shown, hovered) {
      (true, _) => (
        PaletteColor::TextSecondary,
        PaletteColor::TextPrimary,
        TypographyStyle::Label,
      ),
      (false, true) => (
        PaletteColor::TextMuted,
        PaletteColor::TextSecondary,
        TypographyStyle::Description,
      ),
      (false, false) => (
        PaletteColor::TextMuted,
        PaletteColor::TextMuted,
        TypographyStyle::Description,
      ),
    };
    button
      .child(tab_icon(&tab, glyph, &id))
      .child(
        Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
          Truncated::new(format!("{id}-label"), label, style)
            .color(text)
            .render(ctx),
        ),
      )
      .with_children(with_close.then(|| close_button(ctx, closing, &id)))
  }
}

/// The tab's label: *<task key> - <page title>* for an agent's tab, the page title for the person's.
pub fn label(ctx: &Ctx, tab: &StripTab) -> Arc<str> {
  let title = if tab.title.trim().is_empty() {
    if tab.url.is_empty() || tab.url == "about:blank" {
      ctx.t("browser.tabs.blank").to_string()
    } else {
      super::model::shown_address(&tab.url).to_owned()
    }
  } else {
    tab.title.clone()
  };
  match tab.key() {
    Some(key) => ctx.t_args(
      "browser.tabs.agentLabel",
      [("key", key.as_ref()), ("title", title.as_str())],
    ),
    None => Arc::from(title),
  }
}

/// The page's icon with the tab's dot over its corner; the loader while the page loads.
pub fn tab_icon(tab: &StripTab, color: PaletteColor, id: &str) -> Element {
  let glyph = if tab.loading {
    Icon::LoaderCircle
  } else if tab.is_person() {
    Icon::User
  } else {
    Icon::Globe
  };
  let icon = icon(glyph, IconSize::Base, color);
  let Some(indicator) = tab.indicator else {
    return icon.into();
  };
  Stack::new()
    .flex_shrink(0.0)
    .child(icon)
    .child(dot(indicator, format!("{id}-dot")))
    .into()
}

/// The color of a dot.
const fn dot_color(indicator: Indicator) -> PaletteColor {
  match indicator {
    Indicator::Waits => PaletteColor::Warning,
    Indicator::Acting => PaletteColor::Info,
    Indicator::New => PaletteColor::TextPrimary,
  }
}

/// A tab's dot over its icon (`Indicator · …` in `zxkfr`): amber, blue or white, ringed in the title bar's color
/// outside it.
fn dot(indicator: Indicator, id: String) -> Row {
  let ring = BrowserBorder::Ring.value();
  let side = DOT + 2.0 * ring;
  Row::new()
    .id(id)
    .describe("indicator", indicator.name())
    .absolute(
      DOT_LEFT - ring,
      -DOT_RAISE - ring,
      Dimension::Px(side),
      Dimension::Px(side),
    )
    .rounded(ExtraRadius::Full.role())
    .background(dot_color(indicator))
    .border_inside(BrowserBorder::Ring.role(), PaletteColor::SurfaceBase)
}

/// A dot on a line of words: the overflow button's strongest (`⌄ 13 •` in `fWhIr`).
pub fn inline_dot(indicator: Indicator, id: String) -> Row {
  Row::new()
    .id(id)
    .describe("indicator", indicator.name())
    .width(Dimension::Px(DOT - 1.0))
    .height(Dimension::Px(DOT - 1.0))
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Full.role())
    .background(dot_color(indicator))
}

/// The tab's × (`Close` `S5R4Ws`).
fn close_button(
  ctx: &mut Ctx,
  (commands, tab): (super::commands::Commands, orchester_ipc::browser::BrowserTabId),
  id: &str,
) -> Element {
  in_title_bar(Button::empty())
    .id(format!("{id}-close"))
    .describe("a11y_name", ctx.t("browser.tabs.close"))
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .width(Dimension::Px(CLOSE_SIDE))
    .height(Dimension::Px(CLOSE_SIDE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(lurq::layout::layout_kind::Justify::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      commands.send(BrowserCommand::CloseTab { tab: tab.clone() });
    })
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}
