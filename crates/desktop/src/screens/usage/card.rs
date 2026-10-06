//! The dashboard's section card (`C2pTYF`): `$surface` with a `$border` stroke and the panel radius,
//! the title (14/500) with its note at the left and the section's switches at the right, then its
//! body; and what a card says while it reads, has nothing to show, or names a series.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::CliSeries;

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, chart::SWATCH_SIZE},
  ui::{
    icon::{Icon, IconSize, icon},
    segments::{SegmentSize, segments},
  },
};

/// The least height of a card's body while it reads or has nothing to show, so the page keeps its
/// shape (`qQOKw`, `AY6FM`).
const STATE_HEIGHT: f32 = 160.0;

/// A section's header and body.
pub struct Card {
  pub id: &'static str,
  pub title: Arc<str>,
  pub note: Option<Arc<str>>,
  pub trailing: Vec<Element>,
}

impl Card {
  pub fn new(id: &'static str, title: Arc<str>) -> Self {
    Self {
      id,
      title,
      note: None,
      trailing: Vec::new(),
    }
  }

  pub fn note(mut self, note: Arc<str>) -> Self {
    self.note = Some(note);
    self
  }

  pub fn trailing(mut self, element: Element) -> Self {
    self.trailing.push(element);
    self
  }

  pub fn body(self, body: impl IntoIterator<Item = Element>) -> Element {
    let heading = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Mark.role())
      .child(Text::new(&self.title).variant(ExtraTypography::CardTitle.role()))
      .with_children(
        self
          .note
          .map(|note| Element::from(Text::new(&note).variant(TypographyStyle::Caption))),
      );
    Column::new()
      .id(self.id)
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(heading)
          .with_children(self.trailing),
      )
      .with_children(body)
      .into()
  }
}

/// *Chart* / *Table*: every chart can be read as a table with exact numbers.
pub fn view_switch(
  ctx: &Ctx,
  id: &'static str,
  table: bool,
  choose: impl Fn(bool) + Clone + Send + Sync + 'static,
) -> Element {
  segments(
    id,
    &table,
    vec![(false, ctx.t("usage.view.chart")), (true, ctx.t("usage.view.table"))],
    SegmentSize::Compact,
    choose,
  )
}

/// A loader and what is being read, no skeletons (`qQOKw`).
pub fn reading(ctx: &Ctx, scope_name: Option<&str>) -> Element {
  let text = match scope_name {
    Some(project) => ctx.t_args("usage.loading.project", [("project", project)]),
    None => ctx.t("usage.loading.all"),
  };
  centered(
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Loader, IconSize::Button, PaletteColor::TextSecondary))
      .child(Text::new(&text).variant(TypographyStyle::Description))
      .into(),
  )
}

/// What a card says in its chart's place: a title, why, and an action when one helps.
pub fn nothing(title: &str, detail: Option<&str>, action: Option<Element>) -> Element {
  centered(
    Column::new()
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(Text::new(title).variant(TypographyStyle::Label))
      .with_children(detail.map(|detail| Element::from(Text::new(detail).variant(TypographyStyle::Description))))
      .with_children(action)
      .into(),
  )
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
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
pub fn legend_item(color: PaletteColor, name: &str, value: &str, dimmed: bool) -> Element {
  let (name_color, value_color) = if dimmed {
    (PaletteColor::TextMuted, PaletteColor::TextMuted)
  } else {
    (PaletteColor::TextSecondary, PaletteColor::TextPrimary)
  };
  Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(swatch(color))
    .child(
      Text::new(name)
        .variant(TypographyStyle::Caption)
        .color(name_color)
        .nowrap(),
    )
    .child(
      Text::new(value)
        .variant(ExtraTypography::MonoValue.role())
        .color(value_color)
        .nowrap(),
    )
    .into()
}

pub fn swatch(color: PaletteColor) -> Element {
  Row::new()
    .size(Dimension::Px(SWATCH_SIZE), Dimension::Px(SWATCH_SIZE))
    .background(color)
    .rounded(ExtraRadius::Mark.role())
    .into()
}

/// A row of legend entries, then `end` at the right.
pub fn legend(items: Vec<Element>, end: Option<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .with_children(items)
    .child(Spacer::new().flex(1.0))
    .with_children(end)
    .into()
}
