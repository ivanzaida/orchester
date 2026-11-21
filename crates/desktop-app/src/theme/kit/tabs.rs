//! Page tabs (`ui::tabs`, `Page tab/active` `loUWZ`, `Page tab/default` `NspMt`): lurq's `ui::Tabs`, 34 high, 2 at
//! their sides, 20 apart, on a 1 `border` line (`Workflow tabs` `bxhee`). The current tab's label is 13/500
//! `text_primary` over a 2 `accent` underline, the others' 13 `text_secondary`. A count is a 12 `text_muted` number 6
//! after the label, in `warning` where it counts what waits for the person ([`WAITS`]); a badge a mono 11/500 `warning`
//! count on a `warning_muted` pill, 1 over and under and 6 at its sides (`Count badge` `CiYKk`); a dot 6 round (`Dot`
//! `JrkWf`). A tab not built yet is its label in 13 `text_muted` and an 11 `text_muted` lock 5 after it, not faded
//! (`Page tab/locked` `Smb4t`, [`LOCKED`]). A rule between groups is 1 by 14 in `border_strong` (`Rule` `Y3Ixz4`). A
//! compact tab is 32 high ([`COMPACT`], `Tabs` `Cc3JP`). No tab lights up under the pointer. The recipe is the app's
//! whole.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{Length, Recipe, Tabs, TabsPart, TabsState},
};

use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// A tab 32 high, as the Agents hub draws its tabs and a tool's (`Cc3JP`).
pub const COMPACT: &str = "compact";
/// A tab whose count is of what waits for the person, in `warning` (the Agents hub's Memory, `JasTq`).
pub const WAITS: &str = "waits";
/// A tab with a lock after its label: 5 between them.
pub const LOCKED: &str = "locked";

/// `Page tab/*` height.
const TAB_HEIGHT: f32 = 34.0;
/// A compact page tab's height (`Tabs` `Cc3JP`).
const COMPACT_HEIGHT: f32 = 32.0;
/// A tab's dot (`Dot` `JrkWf` in `cjvRr`).
const DOT_SIZE: f32 = 6.0;
/// A count's bone while it is read: about one digit wide, 8 high (`D · Bones` in `13 · Patterns` `AOQZ3`).
const COUNT_BONE: (f32, f32) = (7.0, 8.0);
/// A scroll button of a line too narrow for its tabs (lurq's: no board draws page tabs that overflow).
const SCROLL_BUTTON: f32 = 28.0;
/// The rule between groups of tabs (`Rule` `Y3Ixz4` in `Fbmqe`).
const DIVIDER: (f32, f32) = (1.0, 14.0);

pub(super) fn install(theme: &Theme) {
  use TabsPart::{Label, List, Root, Tab, TrailingIcon};
  use TabsState::{Disabled, Selected};
  let rules = Recipe::new()
    .base(Root, |style| style.width(Length::Fill))
    .base(List, |style| {
      style
        .width(Length::Fill)
        .gap(ExtraSpacing::Tabs.role())
        .underline(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
    .base(Tab, |style| {
      style
        .height(TAB_HEIGHT)
        .padding_x(ExtraSpacing::Tight.role())
        .gap(ExtraSpacing::Snug.role())
        .foreground(PaletteColor::MutedForeground)
        .cursor(CursorIcon::Pointer)
    })
    .state(COMPACT, Tab, |style| style.height(COMPACT_HEIGHT))
    .state(LOCKED, Tab, |style| style.gap(ExtraSpacing::Pair.role()))
    .state(Selected, Tab, |style| {
      style.underline(Border::inside(BorderSize::Md, PaletteColor::Primary))
    })
    .base(Label, |style| {
      style
        .typography(TypographyStyle::Body)
        .foreground(ExtraColor::TextSecondary.role())
    })
    .state(Selected, Label, |style| {
      style
        .typography(ExtraTypography::TabCurrent.role())
        .foreground(PaletteColor::Foreground)
    })
    .base(TrailingIcon, |style| {
      style
        .square(Length::Px(IconSize::Xs.size()))
        .foreground(PaletteColor::MutedForeground)
    });
  let rules = rules
    .state(Disabled, Tab, |style| style.cursor(CursorIcon::NotAllowed))
    .state(Disabled, Label, |style| style.foreground(PaletteColor::MutedForeground));
  theme.set_recipe::<Tabs>(overflowing(marks(rules)));
}

/// What follows a label: its count, badge, bone and dot, and the rule between groups.
fn marks(rules: Recipe) -> Recipe {
  use TabsPart::{Badge, Count, CountSkeleton, Dot, Separator};
  rules
    .base(Count, |style| {
      style
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::MutedForeground)
    })
    .state(WAITS, Count, |style| style.foreground(PaletteColor::Warning))
    .base(Badge, |style| {
      style
        .padding_x(ExtraSpacing::Snug.role())
        .padding_y(ExtraSpacing::Hair.role())
        .radius(ExtraRadius::Full.role())
        .background(PaletteColor::WarningMuted)
        .foreground(PaletteColor::Warning)
        .typography(ExtraTypography::MonoLabel.role())
    })
    .base(Dot, |style| {
      style.square(Length::Px(DOT_SIZE)).radius(ExtraRadius::Full.role())
    })
    .base(CountSkeleton, |style| {
      style.width(Length::Px(COUNT_BONE.0)).height(Length::Px(COUNT_BONE.1))
    })
    .base(Separator, |style| {
      style
        .width(Length::Px(DIVIDER.0))
        .height(Length::Px(DIVIDER.1))
        .background(PaletteColor::Input)
    })
}

/// A line too narrow for its tabs, which no board draws: lurq's overflowing form in the page tabs' roles, the tabs
/// between two scroll buttons and More at the end, on the same line.
fn overflowing(rules: Recipe) -> Recipe {
  use TabsPart::{Bar, List, More, MoreIcon, MoreLabel, ScrollButton, ScrollIcon};
  rules
    .base(Bar, |style| {
      style
        .width(Length::Fill)
        .underline(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
    .state(TabsState::Overflow, List, |style| {
      style
        .width(Length::Auto)
        .underline(Border::inside(BorderSize::Sm, PaletteColor::Border))
    })
    .base(ScrollButton, |style| {
      style
        .width(Length::Px(SCROLL_BUTTON))
        .foreground(PaletteColor::Foreground)
        .cursor(CursorIcon::Pointer)
    })
    .base(ScrollIcon, |style| style.square(Length::Px(IconSize::Sm.size())))
    .base(More, |style| {
      style
        .gap(SpacingSize::Xs)
        .padding_start(SpacingSize::Sm)
        .padding_end(SpacingSize::Xs)
        .start_line(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .foreground(PaletteColor::Foreground)
        .cursor(CursorIcon::Pointer)
    })
    .base(MoreLabel, |style| style.typography(TypographyStyle::Body))
    .base(MoreIcon, |style| {
      style
        .square(Length::Px(IconSize::Sm.size()))
        .foreground(PaletteColor::MutedForeground)
    })
}
