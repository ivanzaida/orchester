//! The window's chrome (`Title bar` `U9Nvc4`): lurq's `ui::WindowChrome` with the app menu and the tabs in its title
//! bar, the window's controls in the platform's order (Windows' at the end, the macOS lights at the start), and the
//! window's frame. The operating system's title bar and border are not shown; the bar's room the app's parts leave
//! drags the window and maximizes it on a double-click. The bar's close control and the macOS close light ask the
//! window to close, as the system's close does, and the window's close handler holds any of them (`lifecycle`).

mod app_menu;
pub mod tab_fit;
mod tab_label;
mod tab_strip;
mod title_bar;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{ChromeBorderPolicy, Row, WindowChromeMode},
  node::{Element, HitTestBehavior, dimension::Dimension},
  ui::{WindowChrome, WindowControls},
};

pub use self::{tab_strip::PillRef, title_bar::Bar};
use crate::theme::ExtraRadius;

/// `Title bar` height, on Windows and macOS alike: what the content's top is under.
pub const TITLE_BAR_HEIGHT: f32 = 36.0;
/// A tab's height, and the button's of the tabs that do not fit (`Tab/*`).
pub const TAB_HEIGHT: f32 = 26.0;
/// The room between the bar's parts (`Gap` `gwMHL`).
pub const BAR_GAP: f32 = 8.0;
/// A Windows control's width (`Minimize` `F195WT`).
pub const CONTROL_WIDTH: f32 = 46.0;

/// The bar's id: its controls are `window-minimize`, `window-maximize` (named *Restore* while the window is maximized)
/// and `window-close`, on macOS the lights' places.
const BAR_ID: &str = "window";

/// The room the bar takes before the app's leading part: none on Windows; on macOS the lights, 12 in, three 12 wide
/// lights 8 apart, and the gap after them.
#[cfg(not(target_os = "macos"))]
pub const LEAD_IN: f32 = 0.0;
#[cfg(target_os = "macos")]
pub const LEAD_IN: f32 = 12.0 + 3.0 * 12.0 + 2.0 * BAR_GAP + BAR_GAP;

/// The room the bar takes after the app's leading part besides the room that drags the window and the trailing part:
/// on Windows the gap and the three controls; on macOS the gaps before and after the drag room (the bar ends at the
/// app's own part).
#[cfg(not(target_os = "macos"))]
pub const END_RESERVE: f32 = BAR_GAP + 3.0 * CONTROL_WIDTH;
#[cfg(target_os = "macos")]
pub const END_RESERVE: f32 = 2.0 * BAR_GAP;

/// The window around `screen`, with the tabs in its title bar; `closing` while the closing panel holds
/// the window.
pub fn chrome(ctx: &mut Ctx, screen: Element, closing: bool) -> Element {
  window(ctx, screen, Bar::Tabs, closing)
}

/// The window around a startup screen: its title bar has the app menu and the window controls only
/// (`Title bar` `WVRwk` of `Window/Signed out`).
pub fn bare(ctx: &mut Ctx, screen: Element) -> Element {
  window(ctx, screen, Bar::Menu, false)
}

fn window(ctx: &mut Ctx, screen: Element, kind: Bar, closing: bool) -> Element {
  let parts = title_bar::content(ctx, (kind, closing));
  framed(ctx, screen, parts)
}

/// A secondary window around `screen` (the Browser window), `leading` at the start of its title bar. Its close
/// controls ask this window to close, as the system's close does.
pub fn secondary(ctx: &mut Ctx, screen: Element, leading: Element) -> Element {
  framed(
    ctx,
    screen,
    title_bar::Parts {
      leading,
      tabs: None,
      trailing: None,
    },
  )
}

fn framed(ctx: &mut Ctx, screen: Element, parts: title_bar::Parts) -> Element {
  let outline = ctx.mount_keyed::<WindowOutline>("window-outline", ());
  let chrome = WindowChrome::new()
    .id(BAR_ID)
    .icon(None)
    .mode(WindowChromeMode::CustomDesktop)
    // lurq's border is four straight strips, which the rounded corners would cut; `WindowOutline` draws the frame
    // instead.
    .border(ChromeBorderPolicy::Hidden)
    .controls(controls(ctx))
    .leading(parts.leading)
    .content(screen)
    .overlay(outline);
  let chrome = match parts.tabs {
    Some(tabs) => chrome.tabs(tabs),
    None => chrome,
  };
  match parts.trailing {
    Some(trailing) => chrome.trailing(trailing),
    None => chrome,
  }
  .mount(ctx)
}

/// The window's controls, named in the app's words.
fn controls(ctx: &mut Ctx) -> WindowControls {
  WindowControls::new()
    .minimize_label(&ctx.t("shell.window.minimize"))
    .maximize_label(&ctx.t("shell.window.maximize"))
    .restore_label(&ctx.t("shell.window.restore"))
    .close_label(&ctx.t("shell.window.close"))
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
      .border_inside(BorderSize::Sm, PaletteColor::Input)
  }
}
