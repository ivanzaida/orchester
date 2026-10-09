//! The custom window chrome (`Title bar` `yZzwy` of `q9mf0k`): the title bar with the app menu and the
//! tabs, the window controls, and the window's frame. The operating system's title bar and border are
//! not shown; the empty parts of the title bar drag the window and maximize it on a double-click.

mod app_menu;
mod tab_fit;
mod tab_strip;
mod title_bar;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{
    ChromeBorderPolicy, ChromeTitleBar, Row, Text, WindowChrome, WindowChromeMode, WindowChromeProps,
    WindowControlContent, WindowControlKind, WindowControls,
  },
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, TextColor, dimension::Dimension},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, IconSize},
  ui::icon::{Icon, glyph as icon_glyph},
};

/// `Title bar` height, on Windows and macOS alike.
const TITLE_BAR_HEIGHT: f32 = 36.0;

/// The window around `screen`.
pub fn chrome(ctx: &mut Ctx, screen: Element) -> Element {
  let bar = ChromeTitleBar::new()
    .background(PaletteColor::SurfaceBase)
    .border_bottom(None)
    .controls(controls(ctx));
  let outline = ctx.mount_keyed::<WindowOutline>("window-outline", ());
  WindowChrome::new()
    .props(
      WindowChromeProps::new()
        .mode(WindowChromeMode::CustomDesktop)
        // lurq's border is four straight strips, which the rounded corners would cut; `WindowOutline`
        // draws the frame instead.
        .border(ChromeBorderPolicy::Hidden)
        .windows_height(TITLE_BAR_HEIGHT)
        .macos_height(TITLE_BAR_HEIGHT),
    )
    .title_bar(title_bar::content(ctx, bar))
    .content(screen)
    .overlay(outline)
    .mount(ctx)
}

/// The window's 1px frame (the `border_strong` stroke of the window frames), rounded like the window's
/// corners. A maximized or full-screen window has no frame.
struct WindowOutline;

impl Component for WindowOutline {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let window = ctx.window().info();
    let outline = Row::new()
      .absolute(0.0, 0.0, Dimension::Pct(100.0), Dimension::Pct(100.0))
      .hit_test(HitTestBehavior::None);
    if window.is_maximized || window.is_full_screen {
      return outline;
    }
    outline
      .rounded(ExtraRadius::Window.role())
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
  }
}

/// The Windows controls (`Window controls` `i3Ckir`): lucide glyphs in `text_secondary`, the `hover` and
/// `pressed` overlays. The design gives the close control no color of its own, so it hovers like the
/// others. Each control's content fills it and carries an id and a name for agents. macOS keeps lurq's
/// traffic lights, which take neither.
fn controls(ctx: &mut Ctx) -> WindowControls {
  let mut controls = WindowControls::new()
    .foreground(PaletteColor::TextSecondary)
    .hover_background(ExtraColor::Hover.role())
    .active_background(ExtraColor::Pressed.role());
  for (kind, id, name_key, glyph, size) in [
    (
      WindowControlKind::Minimize,
      "window-minimize",
      "window.minimize",
      Icon::Minus,
      IconSize::Md,
    ),
    (
      WindowControlKind::Maximize,
      "window-maximize",
      "window.maximize",
      Icon::Square,
      IconSize::Sm,
    ),
    (
      WindowControlKind::Restore,
      "window-restore",
      "window.restore",
      Icon::Copy,
      IconSize::Sm,
    ),
    (
      WindowControlKind::Close,
      "window-close",
      "window.close",
      Icon::X,
      IconSize::Md,
    ),
  ] {
    let name = ctx.t(name_key);
    controls = controls.content(kind, control(id, name, glyph, size));
  }
  controls
}

fn control(id: &'static str, name: Arc<str>, glyph: Icon, size: IconSize) -> WindowControlContent {
  let glyph = icon_glyph(glyph);
  WindowControlContent::element(move |color: TextColor| {
    Row::new()
      .id(id)
      .describe("a11y_name", name.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(Text::new(&glyph).variant(size.role()).color(color))
      .into()
  })
}
