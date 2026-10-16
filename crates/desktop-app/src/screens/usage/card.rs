//! A section card of the Usage page (`Card · Tokens over time` `w1aVOX`): `surface_panel` in a `border`
//! stroke, radius 10, 16 over and under its content and 20 at its sides, its parts 14 apart. Its header is
//! a 28 high line with the title (15/600; 13/600 on an efficiency card) and a note after it, the card's
//! switches at its end, and a subtitle 4 under it. Also what a card says while it reads (`YAWmD`) or has
//! nothing to show (`i9jMW`), the legend entries of its series (`Legend item` `C6OLNt`) and its footnote.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, UsageSpacing, UsageText},
  ui::{
    card::{CardSurface, card_frame},
    icon::{Icon, glyph, icon},
    pressable::{Focus, pressable},
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// A card's header line (`Title row` `WliPs`).
const HEADER_HEIGHT: f32 = 28.0;
/// The room a chart card's body keeps while it reads or shows nothing: the card is then 300 high
/// (`Card · Tokens over time` while reading, `ES85q`) with 16 over and under, the header and 14 under it.
const STATE_ROOM: f32 = 300.0 - 16.0 - HEADER_HEIGHT - 14.0 - 16.0;
/// The widest a state's explanation runs (`voO9U`).
const STATE_MEASURE: f32 = 440.0;
/// A legend's color swatch (`p4TazJ`).
pub const SWATCH: f32 = 8.0;

/// A section's header and body.
pub struct Card {
  id: &'static str,
  title: Arc<str>,
  small: bool,
  note: Option<Arc<str>>,
  subtitle: Option<Arc<str>>,
  trailing: Vec<Element>,
  fill: bool,
  close: bool,
}

impl Card {
  pub fn new(id: &'static str, title: Arc<str>) -> Self {
    Self {
      id,
      title,
      small: false,
      note: None,
      subtitle: None,
      trailing: Vec::new(),
      fill: false,
      close: false,
    }
  }

  /// The smaller title of an efficiency card (`ZlfOf`).
  pub fn small(mut self) -> Self {
    self.small = true;
    self
  }

  /// Words on the title's line after it (*shared by all projects · account-wide*).
  pub fn note(mut self, note: Arc<str>) -> Self {
    self.note = Some(note);
    self
  }

  /// A line under the title (*cached input ÷ all input, per day*).
  pub fn subtitle(mut self, subtitle: Arc<str>) -> Self {
    self.subtitle = Some(subtitle);
    self
  }

  /// A switch or a menu at the end of the title's line.
  pub fn trailing(mut self, element: impl Into<Element>) -> Self {
    self.trailing.push(element.into());
    self
  }

  /// Shares a row with other cards: as tall as the tallest, its body filling what the header leaves.
  pub fn fill(mut self) -> Self {
    self.fill = true;
    self
  }

  /// Its parts 10 apart instead of 14, as a table under its header (`Card · Most expensive` `H5SBl`).
  pub fn close(mut self) -> Self {
    self.close = true;
    self
  }

  pub fn body(self, body: impl IntoIterator<Item = Element>) -> Element {
    let title = Text::new(&self.title)
      .id(format!("{}-title", self.id))
      .variant(if self.small {
        UsageText::SmallTitle.role()
      } else {
        TypographyStyle::Title
      })
      .nowrap();
    let note = self.note.map(|note| {
      Text::new(&note)
        .id(format!("{}-note", self.id))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    });
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(title)
          .with_children(note),
      )
      .with_children(self.trailing);
    let header = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(line)
      .with_children(self.subtitle.map(|subtitle| {
        Text::new(&subtitle)
          .id(format!("{}-subtitle", self.id))
          .variant(UsageText::Subtitle.role())
      }));
    let card = frame(self.id);
    let card = if self.close {
      card.spacing(ExtraSpacing::Loose.role())
    } else {
      card
    }
    .child(header)
    .with_children(body);
    if self.fill { card.flex(1.0) } else { card }.into()
  }
}

/// The card's surface and padding.
fn frame(id: &str) -> Column {
  card_frame(id, CardSurface::Panel)
    .padding(Padding::symmetric(UsageSpacing::CardSide.role(), SpacingSize::Lg))
    .spacing(ExtraSpacing::Inset.role())
}

/// Cards side by side, 16 apart, each as wide and as tall as the others (`Charts` `yuCtS`).
pub fn card_row(id: &str, cards: Vec<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Stretch)
    .with_children(
      cards
        .into_iter()
        .map(|card| Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card)),
    )
    .into()
}

/// *Chart* / *Table*: every chart can be read as a table with exact numbers; `icons` shows the glyphs
/// alone (`Chart / Table` of an efficiency card, `R70D66`).
pub fn view_switch(
  ctx: &Ctx,
  id: &str,
  table: bool,
  icons: bool,
  choose: impl Fn(bool) + Clone + Send + Sync + 'static,
) -> Element {
  let (chart_label, table_label) = (ctx.t("usage.view.chart"), ctx.t("usage.view.table"));
  let face = |label: Arc<str>, glyph: Icon| {
    if icons {
      SegmentFace::Icon(glyph, label)
    } else {
      SegmentFace::Text(label)
    }
  };
  let (to_chart, to_table) = (choose.clone(), choose);
  segmented(
    id,
    [
      Segment::new(
        format!("{id}-chart"),
        face(chart_label, Icon::ChartColumn),
        !table,
        move || to_chart(false),
      ),
      Segment::new(
        format!("{id}-table"),
        face(table_label, Icon::Table),
        table,
        move || to_table(true),
      ),
    ],
    Focus::Pointer,
  )
  .into()
}

/// A loader and what is being read, in the room the chart was going to fill (`Reading` `YAWmD`): no
/// skeletons.
pub fn reading(id: &str, text: &str) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .describe("state", "loading")
    .child(icon(Icon::LoaderCircle, IconSize::Lg, PaletteColor::TextSecondary))
    .child(Text::new(text).variant(TypographyStyle::Description))
    .into()
}

/// What stands in a chart's place (`State` `i9jMW`): its glyph, what it says, why, and an action when one
/// helps, centered with 36 over and under them.
pub fn state(id: &str, (mark, title, detail): (Icon, &str, &str), action: Option<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::symmetric(SpacingSize::Xl, UsageSpacing::State.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .describe("state", "empty")
    .child(
      Text::new(&glyph(mark))
        .variant(UsageText::StateGlyph.role())
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(UsageText::StateTitle.role()),
    )
    .child(
      Text::new(detail)
        .id(format!("{id}-detail"))
        .variant(TypographyStyle::Description)
        .max_width(Dimension::Px(STATE_MEASURE))
        .text_align(lurq::layout::text_style::TextAlign::Center),
    )
    .with_children(action.map(|action| Row::new().padding_top(ExtraSpacing::Snug.role()).child(action)))
    .into()
}

/// A chart card's body that keeps the page's shape while it reads or says why it shows nothing, with
/// `body` in its middle.
pub fn state_room(body: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(STATE_ROOM))
    .justify(Justify::Center)
    .child(body)
    .into()
}

pub fn swatch(color: PaletteColor) -> Element {
  Row::new()
    .size(Dimension::Px(SWATCH), Dimension::Px(SWATCH))
    .flex_shrink(0.0)
    .background(color)
    .rounded(ExtraRadius::Swatch.role())
    .into()
}

/// A footnote under a chart or table.
pub fn footnote(id: &str, text: &str) -> Element {
  Text::new(text)
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .variant(UsageText::Footnote.role())
    .into()
}

/// Pushes what follows to the bottom of a card that fills its row.
pub fn push_down() -> Element {
  Spacer::new().flex(1.0).into()
}

/// Words that open another place, in 12/500 `text_primary` (`Link · Show failed tasks` `ftbjr`).
pub fn text_link(id: &str, label: &str, open: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(open)
    .id(id.to_owned())
    .describe("a11y_name", label.to_owned())
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .into()
}
