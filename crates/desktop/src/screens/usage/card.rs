//! The dashboard's section card (`C2pTYF`): `$surface` with a `$border` stroke and the panel radius;
//! the header holds the title (14/500), what follows it on its line (a legend, a switch, a note),
//! a subtitle under it and the section's switches at the right, then the body. Also what a card
//! says while it reads, has nothing to show, or names a series.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, Breakpoint, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::usage::CliSeries;

use crate::{
  theme::{
    ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography,
    chart::{DASH_PIECES, LINE_SWATCH_HEIGHT, LINE_SWATCH_WIDTH, SWATCH_SIZE},
  },
  ui::{
    icon::{Icon, IconSize, icon},
    link::Press,
    pressable::surface,
    segments::{Segment, SegmentSize, segmented},
  },
};

/// The least height of a card's body while it reads or has nothing to show, so the page keeps its
/// shape (`qQOKw`, `AY6FM`).
const STATE_HEIGHT: f32 = 160.0;
/// The height of the card that stands in for the charts when there is nothing to draw (`AY6FM`,
/// `YxAed`).
const EMPTY_HEIGHT: f32 = 300.0;

/// A section's header and body.
pub struct Card {
  id: &'static str,
  title: Arc<str>,
  lead: Vec<Element>,
  note: Option<Arc<str>>,
  subtitle: Option<Arc<str>>,
  trailing: Vec<Element>,
  fill: bool,
}

impl Card {
  pub fn new(id: &'static str, title: Arc<str>) -> Self {
    Self {
      id,
      title,
      lead: Vec::new(),
      note: None,
      subtitle: None,
      trailing: Vec::new(),
      fill: false,
    }
  }

  /// Something on the title's line after it: a legend or a switch.
  pub fn lead(mut self, element: Element) -> Self {
    self.lead.push(element);
    self
  }

  /// A note on the title's line, after what leads (*shared by all projects · account-wide*).
  pub fn note(mut self, note: Arc<str>) -> Self {
    self.note = Some(note);
    self
  }

  /// A line under the title (*cached input ÷ all input, per day*).
  pub fn subtitle(mut self, subtitle: Arc<str>) -> Self {
    self.subtitle = Some(subtitle);
    self
  }

  pub fn trailing(mut self, element: Element) -> Self {
    self.trailing.push(element);
    self
  }

  /// Shares a row with other cards: as tall as the tallest, its last child kept at the bottom.
  pub const fn fill(mut self) -> Self {
    self.fill = true;
    self
  }

  pub fn body(self, body: impl IntoIterator<Item = Element>) -> Element {
    let note = self.note.map(|note| {
      Element::from(
        Text::new(&note)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
    });
    let title_line = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&self.title)
          .variant(ExtraTypography::CardTitle.role())
          .nowrap(),
      )
      .with_children(self.lead)
      .with_children(note);
    let heading = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Mark.role())
      .child(title_line)
      .with_children(
        self
          .subtitle
          .map(|subtitle| Element::from(Text::new(&subtitle).variant(TypographyStyle::Caption))),
      );
    let header = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(heading)
      .with_children(self.trailing);
    let card = frame(self.id).child(header).with_children(body);
    let card = if self.fill { card.flex(1.0) } else { card };
    card.into()
  }
}

/// The card's surface, stroke, radius and padding.
fn frame(id: &'static str) -> Column {
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .padding_horizontal(SpacingSize::Lg)
    .padding_vertical(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .background(ExtraColor::Surface.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
}

/// A card without a header that stands in for the charts: no runs (`AY6FM`), filters that match
/// nothing (`YxAed`).
pub fn empty_card(id: &'static str, content: Element) -> Element {
  frame(id)
    .min_height(Dimension::Px(EMPTY_HEIGHT))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

/// Cards side by side, each as wide and as tall as the others; under the widest breakpoint there is
/// no room for a chart beside another, so they stack.
pub fn card_row(ctx: &Ctx, cards: Vec<Element>) -> Element {
  if !matches!(ctx.breakpoint(), Some(Breakpoint::Xl)) {
    return Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .with_children(cards)
      .into();
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Stretch)
    .with_children(
      cards
        .into_iter()
        .map(|card| Element::from(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(card))),
    )
    .into()
}

/// *Chart* / *Table*: every chart can be read as a table with exact numbers. `chart` is the glyph
/// of the chart's kind; `compact` shows the glyphs alone (`C2pTYF`, the *Efficiency* cards).
pub fn view_switch(
  ctx: &Ctx,
  (id, chart): (&'static str, Icon),
  table: bool,
  compact: bool,
  choose: impl Fn(bool) + Clone + Send + Sync + 'static,
) -> Element {
  let segment = if compact { Segment::icon } else { Segment::with_icon };
  segmented(
    id,
    &table,
    vec![
      (false, segment(ctx.t("usage.view.chart"), chart)),
      (true, segment(ctx.t("usage.view.table"), Icon::Table)),
    ],
    SegmentSize::Compact,
    choose,
  )
}

/// A loader and what is being read, no skeletons (`qQOKw`).
pub fn reading(text: &str) -> Element {
  centered(
    Column::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Loader, IconSize::Button, PaletteColor::TextSecondary))
      .child(Text::new(text).variant(TypographyStyle::Description))
      .into(),
  )
}

/// What stands in a chart's place: an icon, a title, why, and an action when one helps.
pub fn nothing(glyph: Icon, title: &str, detail: Option<&str>, action: Option<Element>) -> Element {
  centered(state(glyph, title, detail, action))
}

/// The icon, title, explanation and action of a state, centered on each other.
pub fn state(glyph: Icon, title: &str, detail: Option<&str>, action: Option<Element>) -> Element {
  Column::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Button, PaletteColor::TextSecondary))
    .child(Text::new(title).variant(TypographyStyle::Label))
    .with_children(detail.map(|detail| Element::from(Text::new(detail).variant(TypographyStyle::Description))))
    .with_children(action)
    .into()
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(STATE_HEIGHT))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}

/// The color a CLI's series is drawn in.
pub fn series_color(series: CliSeries) -> PaletteColor {
  match series {
    CliSeries::Codex => ExtraColor::ChartSeries1.role(),
    CliSeries::Claude => ExtraColor::ChartSeries2.role(),
    CliSeries::Other => ExtraColor::ChartSeries3.role(),
  }
}

/// A legend entry: the swatch, the name and its value; identity never rests on color alone.
pub fn legend_item(color: PaletteColor, name: &str, value: Option<&str>) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(swatch(color))
    .child(
      Text::new(name)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .with_children(value.map(|value| {
      Element::from(
        Text::new(value)
          .variant(ExtraTypography::MonoValue.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
    }))
    .into()
}

pub fn swatch(color: PaletteColor) -> Element {
  Row::new()
    .size(Dimension::Px(SWATCH_SIZE), Dimension::Px(SWATCH_SIZE))
    .background(color)
    .rounded(ExtraRadius::Mark.role())
    .into()
}

/// A short line in a series color: solid for a weekly window, dashed for a 5-hour one.
pub fn line_swatch(color: &PaletteColor, dashed: bool) -> Element {
  let pieces = if dashed { DASH_PIECES } else { 1 };
  Row::new()
    .width(Dimension::Px(LINE_SWATCH_WIDTH))
    .height(Dimension::Px(LINE_SWATCH_HEIGHT))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Hair.role())
    .with_children((0..pieces).map(|_| {
      Element::from(
        Row::new()
          .flex(1.0)
          .height(Dimension::Px(LINE_SWATCH_HEIGHT))
          .background(color.clone()),
      )
    }))
    .into()
}

/// A row of legend entries, then `caption` after them.
pub fn legend(items: Vec<Element>, caption: Option<Arc<str>>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .wrap()
    .with_children(items)
    .with_children(caption.map(|caption| {
      Element::from(
        Text::new(&caption)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
    }))
    .into()
}

/// A footnote under a chart or table.
pub fn footnote(id: &'static str, text: &str) -> Element {
  Text::new(text)
    .id(id)
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
    .into()
}

/// A text link (*Show all 38 →*, *Show failed tasks →*): `Link` type in `text`, `$hover` under the
/// pointer; its padding widens the press area around the text.
pub fn text_link(id: &'static str, label: &str, press: Press) -> Element {
  press
    .on(surface().id(id))
    .padding_horizontal(SpacingSize::Xs)
    .padding_vertical(ExtraSpacing::Hair.role())
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .child(Text::new(label).variant(TypographyStyle::Link).nowrap())
    .into()
}

/// Pushes what follows to the bottom of a card that fills its row.
pub fn push_down() -> Element {
  Spacer::new().flex(1.0).into()
}
