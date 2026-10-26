//! The frame of Settings (`Content` in `01 · General` `l838Ty` of the new `orchester.pen`): the title and what
//! Settings is on one line with what the open tab's saves did at its end (`Header` `t7cJ0W`), the five page tabs
//! (`Tabs` `xEFQ0`), and the open tab's column of groups under them, 720 wide at most, scrolling (`Body` `EUrDy`,
//! `Pane` `ZbG8J`). A sub-page (an account, a team server, *Add account*, *Connect a server*) opens in the same column
//! with its crumb back to its tab and its own head (`Crumb` `Yi1dp`, `Head` `N0kGb` in `03b · Account`).

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::link::Open,
  theme::{ExtraSpacing, ExtraTypography, IconSize, SettingsSpacing, SettingsText},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    settings_page::FORM_WIDTH,
  },
};

/// The page: its `title` and `sub`, `end` at the header's end, its `tabs`, and `body` under them.
pub(super) fn page(
  id: &str,
  (title, sub): (&str, &str),
  end: Option<Element>,
  tabs: Element,
  body: Element,
) -> Element {
  let header = Row::new()
    .id("settings-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .bottom(SpacingSize::Md)
        .left(SettingsSpacing::Page.role())
        .right(SettingsSpacing::Page.role()),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id("settings-title")
        .variant(TypographyStyle::Heading)
        .nowrap(),
    )
    .child(
      Text::new(sub)
        .id("settings-sub")
        .variant(SettingsText::PageSub.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .with_children(end);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(SettingsSpacing::Page.role()))
        .child(tabs),
    )
    .child(body)
    .into()
}

/// A tab's `pane` scrolling under the tabs in the scroll `id`.
pub(super) fn column(id: &str, pane: impl Into<Element>) -> ScrollVertical {
  ScrollVertical::new(
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SettingsSpacing::BodyTop.role())
          .bottom(SpacingSize::Xl)
          .left(SettingsSpacing::Page.role())
          .right(SettingsSpacing::Page.role()),
      )
      .child(pane),
  )
  .id(id.to_owned())
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0))
}

/// The column of groups (`Pane` `ZbG8J`): [`FORM_WIDTH`] wide at most, 28 between its groups.
pub(super) fn pane(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(FORM_WIDTH))
    .spacing(SettingsSpacing::Page.role())
}

/// The crumb back to a sub-page's tab (`Crumb` `Yi1dp`): an arrow and the tab's name in 12 `text_muted`, 6 apart.
pub(super) fn crumb(id: &str, parent: &str, open: Open) -> Element {
  opening(open)
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", Arc::<str>::from(parent))
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(parent)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// A sub-page's head (`Head` `N0kGb`): an optional tile, its title in 16/600 with a line under it, and its actions at
/// the end, 12 apart.
pub(super) fn head(id: &str, tile: Option<Element>, (title, line): (&str, Option<Element>), actions: Vec<Element>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .with_children(tile)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(SettingsText::SubTitle.role())
            .width(Dimension::Pct(100.0)),
        )
        .with_children(line),
    )
    .with_children(actions)
}

/// A sub-page's lead under its title, in 12.5 `text_secondary` (`Lead` `JY1ng`).
pub(super) fn lead(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
}
