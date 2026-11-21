//! The window's chrome (`Title bar/Windows` `Wl7Kt`, `Window controls/Windows` `vM7fU`, `Window controls/macOS`
//! `x6yXo`): lurq's `ui::WindowChrome`, its bar 36 high on `surface_base` with no line under it and 8 between its
//! parts. On Windows the bar starts at the window's edge (the app menu keeps its own 6) and ends in the three 46 wide
//! controls, lucide glyphs in `text_secondary` (minus 14, square or copy 12, x 14) on the `hover` fill under the
//! pointer and `pressed` held down; the design gives the close control no color of its own, so it does as the others,
//! and a window behind others keeps its glyphs. On macOS the lights stay lurq's (the system's, 12 across, 8 apart, 12
//! in), and the bar ends where the app's part does. No control fades.

use lurq::{
  app::theme::{PaletteColor, Theme},
  node::border::Border,
  ui::{PartStyle, Recipe, Selector, WindowChrome, WindowChromePart, WindowChromeState, WindowPlatform},
};

use crate::{
  chrome::{BAR_GAP, CONTROL_WIDTH, TITLE_BAR_HEIGHT},
  theme::{ExtraColor, IconSize},
};

/// The control parts and their glyphs, with the glyph's size (`IMADK`, `jox0u`, `pswVN`).
const CONTROLS: [(WindowChromePart, WindowChromePart, IconSize); 3] = [
  (
    WindowChromePart::Minimize,
    WindowChromePart::MinimizeGlyph,
    IconSize::Md,
  ),
  (
    WindowChromePart::Maximize,
    WindowChromePart::MaximizeGlyph,
    IconSize::Sm,
  ),
  (WindowChromePart::Close, WindowChromePart::CloseGlyph, IconSize::Md),
];

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<WindowChrome>(|rules| {
    let rules = rules
      .base(WindowChromePart::Bar, |style| {
        style
          .height(TITLE_BAR_HEIGHT)
          .background(PaletteColor::Background)
          .underline(Border::inside(0.0, PaletteColor::Border))
          .gap(BAR_GAP)
      })
      // A bar with tabs is as high as one without, and has no line under its tabs.
      .when(
        Selector::new().state(WindowChromeState::Tabs),
        WindowChromePart::Bar,
        |style| style.height(TITLE_BAR_HEIGHT),
      )
      .base(WindowChromePart::Line, |style| style.height(0.0))
      .when(on(WindowPlatform::Windows), WindowChromePart::Bar, |style| {
        style.padding_start(0.0)
      })
      .when(on(WindowPlatform::Macos), WindowChromePart::Bar, |style| {
        style.padding_end(0.0)
      })
      .when(on(WindowPlatform::Macos), WindowChromePart::Balance, |style| {
        style.width(0.0)
      });
    CONTROLS.into_iter().fold(rules, control)
  });
}

fn on(platform: WindowPlatform) -> Selector {
  Selector::new().variant(platform)
}

/// One Windows control and its glyph, in and out of the states lurq colors them in.
fn control(rules: Recipe, (part, glyph, size): (WindowChromePart, WindowChromePart, IconSize)) -> Recipe {
  let windows = || on(WindowPlatform::Windows);
  let glyph_style = move |style: PartStyle| style.square(size.size()).foreground(ExtraColor::TextSecondary.role());
  rules
    .when(windows(), part, |style| {
      still(style.width(CONTROL_WIDTH).height(TITLE_BAR_HEIGHT))
    })
    .when(windows().hovered(), part, |style| {
      style.background(ExtraColor::Hover.role())
    })
    .when(windows().pressed(), part, |style| {
      style.background(ExtraColor::Pressed.role())
    })
    .when(windows(), glyph, glyph_style)
    .when(windows().state(WindowChromeState::Maximized), glyph, glyph_style)
    .when(windows().state(WindowChromeState::Inactive), glyph, glyph_style)
    .when(windows().within().hovered(), glyph, glyph_style)
    .when(windows().within().pressed(), glyph, glyph_style)
}

/// A control without lurq's fade.
fn still(mut style: PartStyle) -> PartStyle {
  style.transitions = Some(Vec::new());
  style
}
