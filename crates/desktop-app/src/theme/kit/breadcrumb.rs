//! Breadcrumb (`ui::breadcrumb`, `Crumbs` `A7q3o` on the task page, `VpX0E` on New task): lurq's `ui::Breadcrumb`, its
//! steps 6 apart in 12 `text_muted`, the page's own last in `text_secondary`, a `/` in 12 `text_muted` or a 12
//! `chevron-right` between them. A link is underlined under the pointer. A step's own states give a key in Geist Mono
//! ([`MONO`]), the page's title ([`TITLE`]), an ancestor that ends the trail ([`ANCESTOR`]) and a link that ends it
//! ([`LAST`]). The recipe is the app's whole.

use lurq::{
  app::theme::{BorderSize, PaletteColor, Theme, TypographyStyle},
  node::{CursorIcon, border::Border},
  ui::{Breadcrumb, BreadcrumbPart, Length, PartStyle, Recipe},
};

use crate::theme::{ExtraColor, ExtraSpacing, IconSize};

/// A step whose label is an identifier, e.g. a task's key.
pub const MONO: &str = "mono";
/// The page's title in the last step (Effective instructions, `c3yZoo`), in `text_primary`.
pub const TITLE: &str = "title";
/// An ancestor of the page even where it ends the trail, in `text_muted` (`gxF9b` in `o1Uea4`).
pub const ANCESTOR: &str = "ancestor";
/// A link that ends the trail, drawn as the page would be.
pub const LAST: &str = "last";

pub(super) fn install(theme: &Theme) {
  use BreadcrumbPart::{Link, Page, Root, Separator, SeparatorIcon, Text};
  theme.set_recipe::<Breadcrumb>(
    Recipe::new()
      .base(Root, |style| {
        style.gap(ExtraSpacing::Snug.role()).max_width(Length::Fill)
      })
      .base(Link, |style| words(style).cursor(CursorIcon::Pointer))
      .hovered(Link, |style| {
        style.underline(Border::inside(BorderSize::Sm, PaletteColor::MutedForeground))
      })
      .base(Page, |style| words(style).foreground(ExtraColor::TextSecondary.role()))
      .base(Text, words)
      .base(Separator, words)
      .base(SeparatorIcon, |style| style.square(Length::Px(IconSize::Sm.size())))
      .state(LAST, Link, |style| style.foreground(ExtraColor::TextSecondary.role()))
      .state(MONO, Link, |style| style.typography(TypographyStyle::Mono))
      .state(MONO, Page, |style| style.typography(TypographyStyle::Mono))
      .state(MONO, Text, |style| style.typography(TypographyStyle::Mono))
      .state(TITLE, Page, |style| {
        style
          .typography(TypographyStyle::Title)
          .foreground(PaletteColor::Foreground)
      })
      .state(ANCESTOR, Link, |style| style.foreground(PaletteColor::MutedForeground))
      .state(ANCESTOR, Page, |style| style.foreground(PaletteColor::MutedForeground)),
  );
}

/// A step's and a separator's words: 12 `text_muted`.
fn words(style: PartStyle) -> PartStyle {
  style
    .typography(TypographyStyle::Description)
    .foreground(PaletteColor::MutedForeground)
}
