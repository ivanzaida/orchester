//! The project's daily report in the Orchestrator's conversation (ORC-47, `orchester_core::orchestrator::report`;
//! `Report · closed` and `Report · open` in `4 · Report and the rest` `VZKyw`): one line in a `border` frame, radius
//! 10, 40 high, 12 at its sides, 10 apart: a 14 `newspaper`, *Daily report · day* (12.5/500), its totals (a figure in
//! Geist Mono 11.5 in its tone over its words in 12 `text_muted`: merged in `success`, waiting for the person in
//! `warning`, tokens), when it was delivered and the chevron that opens it. Open, its head is on `surface_raised` and
//! its sections show under it, 4 over, 12 at the right and under them and 36 in, 10 apart: what the Orchestrator does
//! with it while *Let the Orchestrator answer agents* is on, then each part a heading (12/600 `text_secondary`) over
//! its items 3 apart ([`items`]): a line 24 high, 8 apart, a 12 icon in its tone, what it is (12.5 `text_primary`)
//! and at its end what it adds (11.5 `text_muted`), which opens its place in the app. The Orchestrator's summary is
//! its answer, under it.

mod items;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{orchestrator::DailyReport, time::Timestamp, tokens::compact_tokens, usage::local_date};
use orchester_workspace::display;

use self::items::{Item, Part};
use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::{opening, pressable},
    tooltip::Tooltip,
  },
};

/// The head's height (`SEujT`).
const HEAD_HEIGHT: f32 = 40.0;
/// An item's height (`cIEuw`).
const ITEM_HEIGHT: f32 = 24.0;

/// The report of a row, `id` its card: the report, when it was kept, whether it started a turn by itself.
#[derive(Clone, PartialEq)]
pub(super) struct ReportProps {
  pub(super) id: String,
  pub(super) report: DailyReport,
  pub(super) at: Option<Timestamp>,
  pub(super) turned: bool,
}

impl DevtoolsInspectable for ReportProps {}

pub(super) struct ReportCard {
  open: Signal<bool>,
}

impl Component for ReportCard {
  type Props = ReportProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReportProps>().clone();
    let id = props.id.as_str();
    let open = self.open.get();
    let sections = open.then(|| sections(ctx, &props));
    Column::new()
      .id(id.to_owned())
      .describe("outcome", "daily_report")
      .describe("day", day_label(&props.report))
      .describe("automatic", props.report.automatic.to_string())
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(head(ctx, &props, &self.open))
      .with_children(sections)
  }
}

/// The head: the day, its totals, when it was delivered and the chevron.
fn head(ctx: &Ctx, props: &ReportProps, open: &Signal<bool>) -> Element {
  let id = props.id.as_str();
  let report = &props.report;
  let shown = open.get();
  let title = ctx.t_args("orchestrator.report.title", [("day", super::day(ctx, report.day))]);
  let total = |suffix: &str, (figure, color): (String, PaletteColor), words: Arc<str>| {
    Row::new()
      .id(format!("{id}-{suffix}"))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(
        Text::new(&figure)
          .variant(ExtraTypography::MonoKey.role())
          .color(color)
          .nowrap(),
      )
      .child(
        Text::new(&words)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
  };
  let toggle = open.clone();
  let row = pressable(move || toggle.update(|open| *open = !*open))
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Newspaper, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&title)
        .id(format!("{id}-title"))
        .variant(OrchestratorText::Emphasis.role())
        .nowrap(),
    )
    .child(total(
      "merged",
      (report.merged.len().to_string(), PaletteColor::Success),
      ctx.t("orchestrator.report.totals.merged"),
    ))
    .child(total(
      "waiting",
      (report.waiting.len().to_string(), PaletteColor::Warning),
      ctx.t("orchestrator.report.totals.waiting"),
    ))
    .child(total(
      "tokens",
      (compact_tokens(report.usage.spent), PaletteColor::TextMuted),
      ctx.t("orchestrator.report.totals.tokens"),
    ))
    .child(Spacer::new().flex(1.0))
    .with_children(props.at.map(|at| {
      Text::new(&delivered(ctx, report, at))
        .id(format!("{id}-state"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
    .child(icon(
      if shown { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ));
  if shown {
    row.background(PaletteColor::SurfaceRaised).into()
  } else {
    row.into()
  }
}

/// The report's sections, 36 in under its head: what the Orchestrator does with it, each part, and when the report
/// read how things stood.
fn sections(ctx: &mut Ctx, props: &ReportProps) -> Element {
  let id = props.id.as_str();
  let effect = props.report.automatic.then(|| {
    let key = if props.turned {
      "orchestrator.report.effectSummary"
    } else {
      "orchestrator.report.effectQueued"
    };
    Text::new(&ctx.t(key))
      .id(format!("{id}-effect"))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  let parts: Vec<Element> = items::parts(ctx, &props.report)
    .into_iter()
    .enumerate()
    .map(|(index, shown)| part(ctx, &format!("{id}-part-{index}"), shown))
    .collect();
  let as_of = ctx.t_args(
    "orchestrator.report.asOf",
    [("time", display::clock(props.report.built))],
  );
  Column::new()
    .id(format!("{id}-sections"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .right(SpacingSize::Md)
        .bottom(SpacingSize::Md)
        .left(OrchestratorSpacing::Report.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .with_children(effect)
    .with_children(parts)
    .child(
      Text::new(&as_of)
        .id(format!("{id}-as-of"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// A part of the report, named `id`: its heading over its items, or what it says without any.
fn part(ctx: &mut Ctx, id: &str, part: Part) -> Element {
  let none = part
    .items
    .is_empty()
    .then_some(part.none)
    .flatten()
    .map(|none| -> Element {
      Text::new(&none)
        .id(format!("{id}-none"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
        .into()
    });
  let items: Vec<Element> = part
    .items
    .into_iter()
    .enumerate()
    .map(|(index, shown)| item(ctx, &format!("{id}-item-{index}"), shown))
    .collect();
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(&part.heading).variant(OrchestratorText::ReportHeading.role()))
    .with_children(items)
    .with_children(none)
    .into()
}

/// An item, named `id`: its icon, what it is and what it adds, opening its place in the app; what more it says in its
/// tooltip.
fn item(ctx: &mut Ctx, id: &str, item: Item) -> Element {
  let content = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(item.glyph, IconSize::Sm, item.tone).flex_shrink(0.0))
    .child(
      Text::new(&item.text)
        .id(format!("{id}-text"))
        .variant(OrchestratorText::Line.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&item.meta)
        .id(format!("{id}-meta"))
        .variant(OrchestratorText::ReportMeta.role())
        .nowrap()
        .flex_shrink(0.0),
    );
  // A link the app does not know shows as text, never as its address.
  let row: Element = match Route::parse(&item.link) {
    Ok(route @ Route::Project(..)) => opening(Navigation::of(ctx).to(route))
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .child(content)
      .into(),
    _ => content.id(id.to_owned()).into(),
  };
  match item.tip {
    Some(tip) => Tooltip::new(format!("{id}-tip"), tip).wrap(ctx, row),
    None => row,
  }
}

/// When `report` was delivered at `at`: its time, and its day when that is a later day than the report's.
fn delivered(ctx: &Ctx, report: &DailyReport, at: Timestamp) -> Arc<str> {
  let time = display::clock(at);
  let on = local_date(at);
  if on == report.day {
    time.into()
  } else {
    ctx.t_args(
      "orchestrator.report.deliveredOn",
      [("day", super::day(ctx, on).to_string()), ("time", time)],
    )
  }
}

/// The report's day as tools read it (`2026-10-26`).
fn day_label(report: &DailyReport) -> String {
  report.day.format("%Y-%m-%d").to_string()
}
