//! The window once the session started: the custom chrome (`QQmsI` on Windows, `V70DN` on macOS)
//! with the tab strip in its title bar, the active tab's screen under it, and the window-wide
//! dialogs (closing with active work, the start-up recovery, discarding a tab's edits).

use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  components::{ChromeBorderPolicy, ChromeTitleBar, Column, Row, WindowChrome, WindowChromeMode, WindowChromeProps},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_ipc::SessionEnd;

use super::{
  Tabs, dirty::DiscardChanges, feed::ProjectFeeds, host::TabHosts, input::window_input, restore::Opening,
  strip::TabStrip,
};
use crate::{
  close::Lifecycle,
  navigation::Places,
  programs::Programs,
  screens::{
    close_dialog::CloseConfirmation,
    startup_recovery::{StartupRecovery, StartupRecoveryProps},
  },
  tasks::integration_jobs::ProjectJobs,
  theme::{ExtraColor, TITLE_BAR_HEIGHT},
  workspace::Workspace,
};

#[derive(Clone, PartialEq)]
pub struct WindowProps {
  pub opening: Opening,
  pub workspace: Workspace,
  pub places: Places,
  pub lifecycle: Lifecycle,
  pub programs: Programs,
  /// How the previous session ended, and when this one started, for the start-up recovery.
  pub previous_end: SessionEnd,
  pub session_start: Timestamp,
  /// The projects' merge and removal operations, shared with Orchester's MCP server.
  pub jobs: ProjectJobs,
}

impl lurq::app::component::DevtoolsInspectable for WindowProps {}

pub struct Window {
  tabs: Tabs,
  feeds: ProjectFeeds,
}

impl Component for Window {
  type Props = WindowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<WindowProps>().clone();
    let tabs = Tabs::new(
      props.opening.set,
      props.places,
      props.workspace,
      props.lifecycle.runtime().clone(),
    );
    Self {
      tabs,
      feeds: ProjectFeeds::new(props.jobs),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Provided on every render: a render from the root replaces the inherited context.
    ctx.provide(self.tabs.clone());
    ctx.provide(self.tabs.dialogs().clone());
    ctx.provide(self.feeds.clone());
    let props = ctx.props::<WindowProps>().clone();
    let recovery = StartupRecoveryProps {
      workspace: props.workspace,
      programs: props.programs,
      previous: props.previous_end,
      session_start: props.session_start,
      shown_project: props.opening.shown_project,
    };
    // The dialogs are modals, so they take no space in this column.
    let screen = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .child(ctx.mount_keyed::<TabHosts>("tab-hosts", ()))
      .child(ctx.mount_keyed::<CloseConfirmation>("close-confirmation", props.lifecycle))
      .child(ctx.mount_keyed::<StartupRecovery>("startup-recovery", recovery))
      .child(ctx.mount_keyed::<DiscardChanges>("discard-changes", ()));
    let strip = ctx.mount_keyed::<TabStrip>("tab-strip", ());
    window_input(&self.tabs, chrome(ctx, strip, screen.into()))
  }
}

/// Custom chrome: no native title bar or border, a canvas-colored title bar holding `title_bar`,
/// whose empty parts drag the window and maximize it on a double-click.
pub fn chrome(ctx: &mut Ctx, title_bar: Element, screen: Element) -> Element {
  WindowChrome::new()
    .props(
      WindowChromeProps::new()
        .mode(WindowChromeMode::CustomDesktop)
        .border(ChromeBorderPolicy::Hidden)
        .windows_height(TITLE_BAR_HEIGHT)
        .macos_height(TITLE_BAR_HEIGHT),
    )
    .title_bar(platform::title_bar(
      ChromeTitleBar::new()
        .background(PaletteColor::SurfaceBase)
        .border_bottom(None)
        .center(title_bar),
    ))
    .content(screen)
    .mount(ctx)
}

/// The area over the sidebar (`D6SIx6`, `R3RtGs`): the tabs start after it on every page.
pub const LEADING_WIDTH: f32 = 216.0;

/// The title bar's area over the sidebar, `width` wide: the window's drag area, with the traffic
/// lights on macOS (`R3RtGs`).
pub fn leading(ctx: &mut Ctx, width: f32) -> Element {
  let area = Row::new()
    .width(Dimension::Px(width))
    .height(Dimension::Px(TITLE_BAR_HEIGHT))
    .align_items(Alignment::Center);
  platform::leading(ctx, area)
}

/// The width the window controls take on the right of the title bar.
pub const fn controls_width() -> f32 {
  platform::CONTROLS_WIDTH
}

/// Windows: minimize, maximize/restore and close on the right, 46 × 32 each.
#[cfg(not(target_os = "macos"))]
mod platform {
  use lurq::{
    app::{ctx::Ctx, theme::PaletteColor},
    components::{ChromeTitleBar, Row, WindowControlKind, WindowControls},
    node::Element,
  };

  use crate::{
    theme::{TITLE_BAR_HEIGHT, WINDOW_CONTROL_WIDTH},
    ui::icon::{Icon, window_control},
  };

  pub(super) const CONTROLS_WIDTH: f32 = 3.0 * WINDOW_CONTROL_WIDTH;

  pub(super) fn title_bar(bar: ChromeTitleBar) -> ChromeTitleBar {
    bar.controls(window_controls())
  }

  pub(super) fn leading(_ctx: &mut Ctx, area: Row) -> Element {
    area.into()
  }

  /// Windows controls as in `QQmsI`: lucide icons in the muted text color and one shared hover
  /// background.
  fn window_controls() -> WindowControls {
    WindowControls::new()
      .button_size(WINDOW_CONTROL_WIDTH, TITLE_BAR_HEIGHT)
      .content(WindowControlKind::Minimize, window_control(Icon::Minus))
      .content(WindowControlKind::Maximize, window_control(Icon::Square))
      .content(WindowControlKind::Restore, window_control(Icon::Copy))
      .content(WindowControlKind::Close, window_control(Icon::X))
      .foreground(PaletteColor::TextSecondary)
      .background(PaletteColor::SurfaceBase)
      .hover_background(PaletteColor::AccentMuted)
      .active_background(PaletteColor::AccentMuted)
  }
}

/// macOS: the traffic lights on the left, inside the area over the sidebar (`R3RtGs`): 12px, 8
/// apart, 12 from the edge, in the platform's colors; the rest of the bar is drag area to the right
/// edge.
#[cfg(target_os = "macos")]
mod platform {
  use lurq::{
    app::{
      ctx::Ctx,
      events::MouseEvent,
      theme::{PaletteColor, SpacingSize},
    },
    components::{ChromeTitleBar, Row},
    node::{CursorIcon, Element, dimension::Dimension},
  };

  use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing};

  pub(super) const CONTROLS_WIDTH: f32 = 0.0;
  /// A traffic light's diameter (`lenEp`).
  const LIGHT_SIZE: f32 = 12.0;

  pub(super) fn title_bar(bar: ChromeTitleBar) -> ChromeTitleBar {
    bar.without_controls()
  }

  pub(super) fn leading(ctx: &mut Ctx, area: Row) -> Element {
    let window = ctx.window();
    let (close, minimize, zoom) = (window.clone(), window.clone(), window.clone());
    let maximized = window.is_maximized;
    area
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Sm)
      .child(light("window-close", ExtraColor::TrafficClose.role(), move || {
        close.close()
      }))
      .child(light(
        "window-minimize",
        ExtraColor::TrafficMinimize.role(),
        move || {
          minimize.set_minimized(true);
        },
      ))
      .child(light("window-zoom", ExtraColor::TrafficZoom.role(), move || {
        zoom.set_maximized(!maximized);
      }))
      .into()
  }

  fn light(id: &'static str, color: PaletteColor, press: impl Fn() + Send + Sync + 'static) -> Row {
    Row::new()
      .id(id)
      .size(Dimension::Px(LIGHT_SIZE), Dimension::Px(LIGHT_SIZE))
      .background(color)
      .rounded(ExtraRadius::Pill.role())
      .cursor(CursorIcon::Pointer)
      .on_mouse_down(|event: MouseEvent| {
        event.prevent_default();
        event.stop_immediate_propagation();
      })
      .on_click(move |event: MouseEvent| {
        press();
        event.prevent_default();
        event.stop_immediate_propagation();
      })
  }
}
