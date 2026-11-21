//! The title bar's tabs (`Tab/Active` `jhz5t`, `Tab/Inactive` `mxo09`, `Tab/Home` `qqzuJ`, `New tab` `qKiBJ`, the
//! tab list button `Dq1Mf`): lurq's `ui::WindowTabs`. A tab is 26 high, radius 6, 10 in at its sides and its parts 6
//! apart: a 13 icon and the label 12, both `text_muted`; the active one is filled with `hover`, its icon
//! `text_secondary`, its label 500 `text_primary`, 6 in at its end after an 18 close button (radius 4, a 12 x in
//! `text_muted`, `hover` under the pointer). The tab at the window's home is its 14 icon alone, 28 wide,
//! `text_secondary` (`text_primary` while active). Tabs stand 2 apart, with no line between them, and *New tab* 2
//! after them; under the pointer a tab takes `hover`. The unsaved dot is 6, `text_secondary`, 6 after the label; a
//! mark stands 6 after it. *New tab* is a 14 plus in `text_muted` in a 26 square, radius 6. The button of the tabs
//! that do not fit is 26 high, 8 before and 6 after, its count in Geist Mono 11 `text_secondary` (`text_primary`
//! under the pointer) 4 before a 12 chevron in `text_muted`; open, it takes `pressed`, its count `text_primary` and
//! its chevron `text_secondary` (`D9NzGQ`). Nothing fades, and the pointer is a hand on what a press does something
//! to.
//!
//! The list of every open tab that button opens (`Tab list` `pB0VB`) is 320 wide on `surface_raised`, radius 10,
//! `border_strong` inside, padding 6, its search field 6 above its rows, which are 2 apart. A row (`Tab list row`
//! `uCIaF`, `zDB5B`) is 28 high, radius 4, 8 before and 4 after its content, 8 apart: a 14 icon in `text_secondary`,
//! its page 12 `text_primary` and its project 12 `text_muted` filling the row, a 12 `eye-off` in `text_muted` while the
//! tab is not in the strip, a 14 check in `text_primary` on the current tab and a 20 close slot whose 12 x in
//! `text_muted` shows under the pointer, `pressed` under it. The current row is `accent_muted`, its label 500 and its
//! icon `text_primary`; a hovered row takes `hover`, the keyboard's row `hover` inside a 1 `border_focus` stroke
//! (`a1i6Ws`). Where nothing matches, 12 `text_muted`, 8 by 10 (`Jmopb`).

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border, color::Color, outline::Outline},
  ui::{PartStyle, Recipe, Selector, WindowTabs, WindowTabsPart as Part, WindowTabsState as State},
};

use crate::{
  chrome::{TAB_HEIGHT, TITLE_BAR_HEIGHT},
  theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
};

/// A close button's square (`Close` `S5R4Ws`).
pub const CLOSE_SIDE: f32 = 18.0;
/// The unsaved dot (`Dirty` `TfKtF` in `Fd1ai`).
pub const DOT_SIDE: f32 = 6.0;
/// *New tab*'s square (`qKiBJ`).
pub const NEW_TAB_SIDE: f32 = 26.0;
/// The list's width (`pB0VB`).
const LIST_WIDTH: f32 = 320.0;
/// A row's close slot (`Close` `CHx8i`).
const ROW_CLOSE_SIDE: f32 = 20.0;
/// No veil over the open button's `pressed` fill.
const NO_VEIL: Color = Color::new(0, 0, 0, 0);

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<WindowTabs>(|rules| rows(list(buttons(marks(tab(rules))))));
}

/// The room between tabs, before the button of the tabs that do not fit and before *New tab* (`Tabs` `mkCf8`).
pub fn gap() -> f32 {
  ExtraSpacing::Tight.value()
}

/// The strip and a tab: its fill, its icon and its label.
fn tab(rules: Recipe) -> Recipe {
  rules
    .base(Part::Root, |style| style.height(TITLE_BAR_HEIGHT).gap(gap()))
    .base(Part::List, |style| style.height(TITLE_BAR_HEIGHT).gap(gap()))
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
    // No line between tabs: a separator 0 wide is not laid out.
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
    .state(State::MenuOpen, Part::More, |style| {
      style.background(ExtraColor::Pressed.role()).veil(NO_VEIL)
    })
    .state(State::MenuOpen, Part::MoreLabel, |style| {
      style.foreground(PaletteColor::Foreground)
    })
    .state(State::MenuOpen, Part::MoreIcon, |style| {
      style.foreground(ExtraColor::TextSecondary.role())
    })
}

/// The list of every open tab: its panel, its search field, and what it says where nothing matches.
fn list(rules: Recipe) -> Recipe {
  rules
    .base(Part::Panel, |style| {
      style
        .width(LIST_WIDTH)
        .padding_x(ExtraSpacing::Snug.role())
        .padding_y(ExtraSpacing::Snug.role())
        .gap(ExtraSpacing::Tight.role())
        .radius(RadiusSize::Lg)
        .background(PaletteColor::Card)
        .foreground(PaletteColor::Foreground)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
        .shadow(ShadowStyle::Lg)
    })
    .base(Part::Search, |style| style.padding_bottom(SpacingSize::Xs))
    .base(Part::Items, |style| style.gap(ExtraSpacing::Tight.role()))
    // lurq's rows abut under the search field; the design's stay 2 apart.
    .state(State::Searchable, Part::Items, |style| {
      style.gap(ExtraSpacing::Tight.role())
    })
    .base(Part::Empty, |style| {
      style
        .padding_x(ExtraSpacing::Loose.role())
        .padding_y(SpacingSize::Sm)
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::MutedForeground)
    })
}

/// A row of the list: its fill, its icon, its label, its marks and its close button.
fn rows(rules: Recipe) -> Recipe {
  rules
    .base(Part::Item, |style| {
      still(
        style
          .height(ControlHeight::Md.value())
          .padding_start(SpacingSize::Sm)
          .padding_end(SpacingSize::Xs)
          .gap(SpacingSize::Sm)
          .radius(RadiusSize::Sm)
          .foreground(PaletteColor::Foreground)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::Item, |style| style.background(ExtraColor::Hover.role()))
    .focus_visible(Part::Item, |style| {
      style
        .background(ExtraColor::Hover.role())
        .outline(Outline::new(BorderSize::Sm, PaletteColor::Ring).offset(-1.0))
    })
    .state(State::Selected, Part::Item, |style| {
      style.background(PaletteColor::Highlight)
    })
    .when(Selector::new().state(State::Selected).hovered(), Part::Item, |style| {
      style.background(PaletteColor::Highlight)
    })
    .base(Part::ItemIcon, |style| {
      style
        .square(IconSize::Md.size())
        .foreground(ExtraColor::TextSecondary.role())
    })
    .state(State::Selected, Part::ItemIcon, |style| {
      style.foreground(PaletteColor::Foreground)
    })
    .base(Part::ItemLabel, |style| {
      style
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::Foreground)
    })
    .state(State::Selected, Part::ItemLabel, |style| {
      style.typography(TypographyStyle::Label)
    })
    .base(Part::ItemDetail, |style| {
      style
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::MutedForeground)
    })
    .base(Part::ItemHidden, |style| {
      style
        .square(IconSize::Sm.size())
        .foreground(PaletteColor::MutedForeground)
    })
    .base(Part::ItemCurrent, |style| {
      style.square(IconSize::Md.size()).foreground(PaletteColor::Foreground)
    })
    .base(Part::ItemClose, |style| {
      still(
        style
          .square(ROW_CLOSE_SIDE)
          .radius(RadiusSize::Sm)
          .cursor(CursorIcon::Pointer),
      )
    })
    .hovered(Part::ItemClose, |style| style.background(ExtraColor::Pressed.role()))
    .base(Part::ItemCloseIcon, |style| {
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
