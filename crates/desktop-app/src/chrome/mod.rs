//! The custom window chrome (`Title bar` `yZzwy` of `q9mf0k`): the title bar with the app menu and the
//! tabs (the mark alone on the sign-in gate, `dd04F` in `Lpznn`), the window controls, and the window's frame. The
//! operating system's title bar and border are not shown; the empty parts of the title bar drag the window and maximize
//! it on a double-click.

mod app_menu;
pub mod tab_fit;
mod tab_label;
mod tab_list;
mod tab_measure;
mod tab_strip;
mod title_bar;
mod traffic_lights;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor},
  },
  components::{
    ChromeBorderPolicy, ChromeTitleBar, Row, Text, WindowChrome, WindowChromeMode, WindowChromeProps,
    WindowControlContent, WindowControlKind, WindowControlStyle, WindowControls,
  },
  layout::{Alignment, layout_kind::Justify},
  node::{Element, HitTestBehavior, TextColor, dimension::Dimension},
};

pub use self::{tab_strip::PillRef, title_bar::Bar};
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraRadius, IconSize},
  ui::icon::{Icon, glyph as icon_glyph},
};

/// `Title bar` height, on Windows and macOS alike: what the content's top is under.
pub const TITLE_BAR_HEIGHT: f32 = 36.0;

/// The window around `screen`, with the tabs in its title bar; `closing` while the closing panel holds
/// the window.
pub fn chrome(ctx: &mut Ctx, screen: Element, closing: bool) -> Element {
  window(ctx, screen, Bar::Tabs, closing)
}

/// The window around a startup screen: its title bar has the app menu and the window controls only
/// (`Title bar` `bcDe3` of `N0Lpc`).
pub fn bare(ctx: &mut Ctx, screen: Element) -> Element {
  window(ctx, screen, Bar::Menu, false)
}

/// The window around the sign-in gate: its title bar has the mark and the window controls only, no menu (`Title
/// bar` `dd04F` of `Lpznn`); `closing` while the closing panel holds the window.
pub fn gate(ctx: &mut Ctx, screen: Element, closing: bool) -> Element {
  window(ctx, screen, Bar::Mark, closing)
}

fn window(ctx: &mut Ctx, screen: Element, kind: Bar, closing: bool) -> Element {
  let close = close(ctx);
  framed(ctx, screen, close, |ctx, bar, lights| {
    title_bar::content(ctx, bar, (kind, closing), lights)
  })
}

/// A secondary window around `screen` (the Browser window), its title bar's content from `content`, given the bar
/// with the window controls and on macOS the traffic lights. Its close controls close only this window, as the
/// system's close does.
pub fn secondary(
  ctx: &mut Ctx,
  screen: Element,
  content: impl FnOnce(&mut Ctx, ChromeTitleBar, Option<Element>) -> ChromeTitleBar,
) -> Element {
  let window = ctx.window();
  framed(ctx, screen, Arc::new(move || window.request_close()), content)
}

fn framed(
  ctx: &mut Ctx,
  screen: Element,
  close: Close,
  content: impl FnOnce(&mut Ctx, ChromeTitleBar, Option<Element>) -> ChromeTitleBar,
) -> Element {
  let controls = controls(ctx, &close);
  let lights = cfg!(target_os = "macos")
    .then(|| ctx.mount_keyed::<traffic_lights::TrafficLights>("traffic-lights", traffic_lights::LightsProps(close)));
  let bar = ChromeTitleBar::new()
    .background(PaletteColor::SurfaceBase)
    .border_bottom(None)
    .controls(controls);
  let outline = ctx.mount_keyed::<WindowOutline>("window-outline", ());
  let title_bar = content(ctx, bar, lights);
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
    .title_bar(title_bar)
    .content(screen)
    .overlay(outline)
    .mount(ctx)
}

/// What the window's own close controls do: ask to close, as the system's close does.
pub(super) type Close = Arc<dyn Fn() + Send + Sync>;

/// The close of a started session goes to its close flow, which may hold it (`lifecycle`) and closes the
/// window once it may; a startup screen's window closes as the system asks.
fn close(ctx: &mut Ctx) -> Close {
  let window = ctx.window();
  match ctx.use_context::<Services>() {
    Some(services) => Arc::new(move || {
      let window = window.clone();
      services.close.request(move || window.close());
    }),
    None => Arc::new(move || window.request_close()),
  }
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
/// others. Each control's content fills it and carries an id and a name for agents. lurq's close control
/// closes the window outright after its handler; the close control's content takes the press first and
/// asks to close instead, which the window's close flow can hold (`lifecycle`). On macOS lurq's traffic
/// lights have the same forced close and no content of their own, so the app draws them
/// ([`traffic_lights`]) and lurq's are hidden.
fn controls(ctx: &mut Ctx, close: &Close) -> WindowControls {
  let style = if cfg!(target_os = "macos") {
    WindowControlStyle::Hidden
  } else {
    WindowControlStyle::Windows
  };
  let mut controls = WindowControls::new()
    .style(style)
    .foreground(PaletteColor::TextSecondary)
    .hover_background(ExtraColor::Hover.role())
    .active_background(ExtraColor::Pressed.role());
  for (kind, id, name_key, glyph, size) in [
    (
      WindowControlKind::Minimize,
      "window-minimize",
      "shell.window.minimize",
      Icon::Minus,
      IconSize::Md,
    ),
    (
      WindowControlKind::Maximize,
      "window-maximize",
      "shell.window.maximize",
      Icon::Square,
      IconSize::Sm,
    ),
    (
      WindowControlKind::Restore,
      "window-restore",
      "shell.window.restore",
      Icon::Copy,
      IconSize::Sm,
    ),
    (
      WindowControlKind::Close,
      "window-close",
      "shell.window.close",
      Icon::X,
      IconSize::Md,
    ),
  ] {
    let name = ctx.t(name_key);
    let press = (kind == WindowControlKind::Close).then(|| close.clone());
    controls = controls.content(kind, control(id, name, (glyph, size), press));
  }
  controls
}

/// A control's content; the close control's (`close`) asks the window to close.
fn control(
  id: &'static str,
  name: Arc<str>,
  (glyph, size): (Icon, IconSize),
  close: Option<Close>,
) -> WindowControlContent {
  let glyph = icon_glyph(glyph);
  WindowControlContent::element(move |color: TextColor| {
    let content = Row::new()
      .id(id)
      .describe("a11y_name", name.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(Text::new(&glyph).variant(size.role()).color(color));
    match close.clone() {
      Some(close) => content
        .on_click(move |event: MouseEvent| {
          close();
          event.prevent_default();
          event.stop_immediate_propagation();
        })
        .into(),
      None => content.into(),
    }
  })
}
