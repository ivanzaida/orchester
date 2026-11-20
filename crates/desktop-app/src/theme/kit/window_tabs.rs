//! The title bar's tabs (`Tab/Active` `jhz5t`, `Tab/Inactive` `mxo09`, `Tab/Home` `qqzuJ`, `New tab` `qKiBJ`, the
//! tab list button `Dq1Mf`): lurq's `ui::WindowTabs`. A tab is 26 high, radius 6, 10 in at its sides and its parts 6
//! apart: a 13 icon and the label 12, both `text_muted`; the active one is filled with `hover`, its icon
//! `text_secondary`, its label 500 `text_primary`, 6 in at its end after an 18 close button (radius 4, a 12 x in
//! `text_muted`, `hover` under the pointer). The tab at the window's home is its 14 icon alone, 28 wide,
//! `text_secondary` (`text_primary` while active). Tabs abut, with no line between them; under the pointer a tab takes
//! `hover`. The unsaved dot is 6, `text_secondary`, 6 after the label; a mark stands 6 after it. *New tab*
//! is a 14 plus in `text_muted` in a 26 square, radius 6. The button of the tabs that do not fit is 26 high, 8 before
//! and 6 after, its count in Geist Mono 11 `text_secondary` (`text_primary` under the pointer) 4 before a 12
//! chevron in `text_muted`. Nothing fades, and the pointer is a hand on what a press does something to.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{PartStyle, Recipe, Selector, WindowTabs, WindowTabsPart as Part, WindowTabsState as State},
};

use crate::{
  chrome::{TAB_HEIGHT, TITLE_BAR_HEIGHT},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
};

/// A close button's square (`Close` `S5R4Ws`).
pub const CLOSE_SIDE: f32 = 18.0;
/// The unsaved dot (`Dirty` `TfKtF` in `Fd1ai`).
pub const DOT_SIDE: f32 = 6.0;
/// *New tab*'s square (`qKiBJ`).
pub const NEW_TAB_SIDE: f32 = 26.0;

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<WindowTabs>(|rules| buttons(marks(tab(rules))));
}

/// The strip and a tab: its fill, its icon and its label.
fn tab(rules: Recipe) -> Recipe {
  rules
    .base(Part::Root, |style| style.height(TITLE_BAR_HEIGHT))
    .base(Part::List, |style| style.height(TITLE_BAR_HEIGHT))
    .base(Part::Tab, |style| {
      still(
        style
          .height(TAB_HEIGHT)
          .min_width(0.0)
          .radius(RadiusSize::Md)
          .padding_start(ExtraSpacing::Loose.role())
          .padding_end(ExtraSpacing::Loose.role())
          .gap(ExtraSpacing::Snug.role())
          .foreground(PaletteColor::MutedForeground)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::Tab, |style| style.background(ExtraColor::Hover.role()))
    .state(State::Selected, Part::Tab, |style| {
      let none = || Border::inside(0.0, PaletteColor::Border);
      style
        .background(ExtraColor::Hover.role())
        .foreground(PaletteColor::Foreground)
        .top_line(none())
        .start_line(none())
        .end_line(none())
        .padding_end(ExtraSpacing::Snug.role())
    })
    .when(Selector::new().state(State::Selected).hovered(), Part::Tab, |style| {
      style.background(ExtraColor::Hover.role())
    })
    .base(Part::Icon, |style| {
      style
        .square(IconSize::Base.size())
        .foreground(PaletteColor::MutedForeground)
    })
    .state(State::Selected, Part::Icon, |style| {
      style.foreground(ExtraColor::TextSecondary.role())
    })
    .state(State::IconOnly, Part::Icon, |style| {
      style
        .square(IconSize::Md.size())
        .foreground(ExtraColor::TextSecondary.role())
    })
    .when(
      Selector::new().state(State::IconOnly).state(State::Selected),
      Part::Icon,
      |style| style.foreground(PaletteColor::Foreground),
    )
    .base(Part::Label, |style| {
      style
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::MutedForeground)
    })
    .state(State::Selected, Part::Label, |style| {
      style
        .typography(TypographyStyle::Label)
        .foreground(PaletteColor::Foreground)
    })
}

/// What stands after a tab's label: its unsaved dot and its mark, and its close button.
fn marks(rules: Recipe) -> Recipe {
  rules
    .base(Part::Title, |style| style.gap(ExtraSpacing::Snug.role()))
    .base(Part::Marks, |style| style.gap(SpacingSize::Xs))
    .base(Part::Close, |style| {
      still(
        style
          .square(CLOSE_SIDE)
          .radius(RadiusSize::Sm)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::Close, |style| style.background(ExtraColor::Hover.role()))
    .base(Part::CloseIcon, |style| {
      style
        .square(IconSize::Sm.size())
        .foreground(PaletteColor::MutedForeground)
        .opacity(1.0)
    })
    .base(Part::Dot, |style| {
      style
        .square(DOT_SIDE)
        .radius(ExtraRadius::Full.role())
        .background(ExtraColor::TextSecondary.role())
    })
}

/// *New tab*, and the button of the tabs that do not fit.
fn buttons(rules: Recipe) -> Recipe {
  rules
    // No line between tabs. The design's 2 between them has no place: lurq counts a strip's width from its tabs,
    // its separators and its add button, not from a gap, and a separator only of the tabs in the strip, so a tab
    // that left would not come back.
    .base(Part::Separator, |style| style.width(0.0).height(0.0))
    .base(Part::Add, |style| {
      still(
        style
          .width(NEW_TAB_SIDE)
          .height(NEW_TAB_SIDE)
          .radius(RadiusSize::Md)
          .foreground(PaletteColor::MutedForeground)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::Add, |style| style.background(ExtraColor::Hover.role()))
    .base(Part::AddIcon, |style| style.square(IconSize::Md.size()))
    .base(Part::More, |style| {
      still(
        style
          .height(TAB_HEIGHT)
          .padding_start(SpacingSize::Sm)
          .padding_end(ExtraSpacing::Snug.role())
          .gap(SpacingSize::Xs)
          .radius(RadiusSize::Md)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::More, |style| style.background(ExtraColor::Hover.role()))
    .base(Part::MoreLabel, |style| {
      style
        .typography(ExtraTypography::MonoNote.role())
        .foreground(ExtraColor::TextSecondary.role())
    })
    .within_hovered(Part::MoreLabel, |style| style.foreground(PaletteColor::Foreground))
    .base(Part::MoreIcon, |style| {
      style
        .square(IconSize::Sm.size())
        .foreground(PaletteColor::MutedForeground)
    })
}

/// A part without lurq's fade.
fn still(mut style: PartStyle) -> PartStyle {
  style.transitions = Some(Vec::new());
  style
}
