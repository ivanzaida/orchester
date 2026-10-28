//! *Delivery* (`Delivery` `YkDqG` in 01 `LXhun`): each open module by target date, up to three, its target mark, name
//! and *12 / 18 tasks*, a 4 high meter of what is done, and its forecast (*Likely Nov 2 · 85% by Nov 4*, or why there
//! is none); then the queued work with its 85% date. *Delivery ↗* opens the Delivery view. Read off the UI thread
//! (`orchester_workspace::overview::delivery`) when the project's tasks were saved, never per frame; a reading that
//! fails stands in the section's place with Try again (05 `JzNRO`).

use std::path::PathBuf;

use chrono::{Datelike, NaiveDate};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{
  background,
  delivery::{DeliveryPeriod, Forecast, Outlook},
  display::MONTH_KEYS,
  messages::{self, Message},
  modules::measure::WorkUnit,
  overview::{MODULES_SHOWN, delivery::OverviewDelivery},
};

use super::section::{SIDE_LINE_HEIGHT, head, line, link, section};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OverviewText},
  ui::{
    failure::{Failure, LastFailure},
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
  },
};

/// A meter's height (`O4fmlz`).
const METER_HEIGHT: f32 = 4.0;

pub(super) fn delivery(ctx: &mut Ctx, project: ProjectId, root: PathBuf, saves: u64) -> Element {
  ctx.mount_keyed::<DeliverySection>("overview-delivery", DeliveryProps { project, root, saves })
}

#[derive(Clone, PartialEq)]
struct DeliveryProps {
  project: ProjectId,
  root: PathBuf,
  /// Every save of the project's tasks: a merge is one, and the forecasts are read again after it.
  saves: u64,
}

impl DevtoolsInspectable for DeliveryProps {}

struct DeliverySection {
  tries: Signal<u64>,
  last: LastFailure,
}

impl Component for DeliverySection {
  type Props = DeliveryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      tries: ctx.signal(0),
      last: LastFailure::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let DeliveryProps { project, root, saves } = ctx.props::<DeliveryProps>().clone();
    let read = ctx.future((root, saves, self.tries.get()), |(root, ..)| async move {
      background::run(move || OverviewDelivery::read_in(&root, Timestamp::now()))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's delivery forecasts");
          messages::store_error(&error)
        })
    });
    let state = read.state().get();
    let failed = self.last.follow(&state);
    let open = Navigation::of(ctx).to(Route::Project(
      project,
      ProjectPage::Delivery(DeliveryPeriod::default()),
    ));
    let section = section("overview-delivery").child(head(
      "overview-delivery",
      &ctx.t("overview.delivery.title"),
      None,
      Some(link(
        "overview-delivery-open",
        &ctx.t("overview.delivery.link"),
        Press::Open(open),
      )),
    ));
    if let Some((error, retrying)) = failed {
      let tries = self.tries.clone();
      return section.child(
        Failure::new("overview-delivery-failed", ctx.t("overview.delivery.failed"), error)
          .still_works(ctx.t("overview.delivery.stillWorks"))
          .retry(retrying, move || tries.update(|tries| *tries += 1))
          .section(ctx),
      );
    }
    let Some(delivery) = state.data else {
      return section;
    };
    section
      .with_children(delivery.modules.iter().take(MODULES_SHOWN).map(|module| {
        let unit = match module.progress.unit {
          WorkUnit::Tasks => "overview.delivery.tasks",
        };
        let done = ctx.t_args(
          unit,
          [
            ("done", module.progress.done.to_string()),
            ("scope", module.progress.scope.to_string()),
          ],
        );
        let percent = module.progress.percent().unwrap_or(0);
        let item = Item {
          id: format!("overview-delivery-{}", module.id),
          name: &module.name,
          done: &done,
          percent,
          forecast: &forecast(ctx, &module.forecast),
        };
        item.render()
      }))
      .child(queued(ctx, &delivery.queued))
  }
}

/// A module's line, meter and forecast.
struct Item<'a> {
  id: String,
  name: &'a str,
  done: &'a str,
  percent: u32,
  forecast: &'a str,
}

impl Item<'_> {
  fn render(self) -> Element {
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Target, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(self.name)
          .variant(OverviewText::Line.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(Spacer::new().flex(1.0))
      .child(Text::new(self.done).variant(ExtraTypography::MonoKey.role()).nowrap());
    // A whole percent of a few hundred pixels: f32 is precise enough.
    let percent = self.percent.min(100) as f32;
    let meter = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(METER_HEIGHT))
      .background(PaletteColor::Border)
      .rounded(ExtraRadius::Meter.role())
      .child(
        Column::new()
          .width(Dimension::Pct(percent))
          .height(Dimension::Pct(100.0))
          .background(PaletteColor::TextSecondary)
          .rounded(ExtraRadius::Meter.role()),
      );
    Column::new()
      .id(self.id.clone())
      .describe("done", self.percent.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
      .spacing(ExtraSpacing::Snug.role())
      .child(line)
      .child(meter)
      .child(
        Text::new(self.forecast)
          .id(format!("{}-forecast", self.id))
          .variant(ExtraTypography::Note.role())
          .nowrap(),
      )
      .into()
  }
}

/// *Queued work · 3 tasks* with its 85% date.
fn queued(ctx: &Ctx, forecast: &Forecast) -> Row {
  let count = forecast.remaining;
  let words = if count == 1 {
    ctx.t("overview.delivery.queued.one")
  } else {
    ctx.t_args("overview.delivery.queued.other", [("count", count.to_string())])
  };
  let when = match &forecast.outlook {
    Outlook::Dates { safe, .. } => ctx.t_args("overview.delivery.queuedSafe", [("date", date(ctx, *safe))]),
    Outlook::Done => ctx.t("overview.delivery.done"),
    Outlook::NotEnoughHistory => "".into(),
  };
  line("overview-delivery-queued", SIDE_LINE_HEIGHT, SpacingSize::Sm)
    .child(icon(Icon::ListOrdered, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(OverviewText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .flex(1.0),
    )
    .child(
      Text::new(&when)
        .id("overview-delivery-queued-when")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
}

/// A forecast in words: *Likely Nov 2 · 85% by Nov 4*, *Done*, or why there is none.
fn forecast(ctx: &Ctx, forecast: &Forecast) -> String {
  match &forecast.outlook {
    Outlook::Dates { likely, safe, .. } => ctx
      .t_args(
        "overview.delivery.dates",
        [("likely", date(ctx, *likely)), ("safe", date(ctx, *safe))],
      )
      .to_string(),
    Outlook::Done => ctx.t("overview.delivery.done").to_string(),
    Outlook::NotEnoughHistory => ctx
      .t_args(
        "overview.delivery.notEnough",
        [
          ("tasks", forecast.history_delivered.to_string()),
          ("days", forecast.history_days.to_string()),
        ],
      )
      .to_string(),
  }
}

/// *Nov 2*.
fn date(ctx: &Ctx, date: NaiveDate) -> String {
  let month = message_text(ctx, &Message::new(MONTH_KEYS[date.month0() as usize]));
  ctx
    .t_args(
      "overview.delivery.date",
      [("month", month.to_string()), ("day", date.day().to_string())],
    )
    .to_string()
}
