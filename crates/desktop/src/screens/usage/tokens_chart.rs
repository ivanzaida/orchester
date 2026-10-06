//! *Tokens over time* (`C2pTYF`): stacked bars by CLI per hour or day with *Spent* / *Cached* /
//! *Output*, a legend with each CLI's total, a tooltip on the hovered bar (`BLY4R`: the day, each
//! CLI's exact count with its swatch, spent and runs) and the same data as a table (`CDcij`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  canvas::CanvasObserver,
  components::{Canvas, Column, Row, Spacer, Stack, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{CliSeries, Grouping, Measure, SeriesBar, local_date};
use parking_lot::Mutex;

use super::{
  card::{Card, legend, legend_item, series_color, swatch, view_switch},
  format,
  plot::{Frame, label_every},
  state::{Dashboard, Views},
  table::{Cell, TableColumn, Width, column, row, table},
  tokens_draw::{TokenPlot, draw},
};
use crate::{
  theme::{
    ExtraRadius, ExtraShadow, ExtraSpacing,
    chart::{ChartStyle, TOKENS_PLOT_HEIGHT, TOOLTIP_OFFSET, TOOLTIP_WIDTH},
  },
  ui::segments::{SegmentSize, segments},
};

/// About how wide an x axis label is, to leave out labels that would crowd.
const LABEL_ROOM: f32 = 56.0;

#[derive(Clone)]
pub struct TokensProps {
  pub dashboard: Arc<Dashboard>,
  pub views: Signal<Views>,
}

impl PartialEq for TokensProps {
  fn eq(&self, other: &Self) -> bool {
    self.dashboard == other.dashboard
  }
}

impl lurq::app::component::DevtoolsInspectable for TokensProps {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Hovered(Option<usize>);

impl lurq::app::component::DevtoolsInspectable for Hovered {}

pub struct TokensChart {
  reference: ElementRef,
  hovered: Signal<Hovered>,
  /// What the canvas shows, for the redraws its size changes ask for.
  plot: Arc<Mutex<Option<TokenPlot>>>,
  observer: Mutex<Option<CanvasObserver>>,
}

impl Component for TokensChart {
  type Props = TokensProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      hovered: ctx.signal(Hovered::default()),
      plot: Arc::new(Mutex::new(None)),
      observer: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TokensProps>().clone();
    let views = props.views.get();
    let (measures, tables) = (props.views.clone(), props.views.clone());
    let measure = segments(
      "usage-tokens-measure",
      &views.measure,
      vec![
        (Measure::Spent, ctx.t("usage.measure.spent")),
        (Measure::Cached, ctx.t("usage.measure.cached")),
        (Measure::Output, ctx.t("usage.measure.output")),
      ],
      SegmentSize::Compact,
      move |measure| measures.update(|views| views.measure = measure),
    );
    let switch = view_switch(ctx, "usage-tokens-view", views.tokens_table, move |table| {
      tables.update(|views| views.tokens_table = table);
    });
    let card = Card::new("usage-tokens", ctx.t("usage.tokens.title"))
      .trailing(measure)
      .trailing(switch);
    let dashboard = &props.dashboard;
    if views.tokens_table {
      return card.body([tokens_table(ctx, dashboard, views.measure)]);
    }
    card.body([
      self.chart(ctx, dashboard, views.measure),
      totals(ctx, dashboard, views.measure),
    ])
  }

  fn after_layout(&self) {
    let mut observer = self.observer.lock();
    if observer.is_none()
      && let Some(canvas) = self.reference.as_canvas()
    {
      let (target, plot) = (canvas.clone(), Arc::clone(&self.plot));
      *observer = Some(canvas.observe_metrics(move |_| {
        if let Some(plot) = plot.lock().as_ref() {
          draw(&target, plot);
        }
      }));
    }
  }
}

impl TokensChart {
  fn chart(&self, ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
    let bars = &dashboard.report.series;
    let hovered = self.hovered.get().0.filter(|index| *index < bars.len());
    let width = self.reference.as_canvas().map(|canvas| canvas.size().width);
    let plot = TokenPlot {
      bars: bars
        .iter()
        .map(|bar| bar.by_cli.map(|usage| measure.of(usage)))
        .collect(),
      labels: labels(ctx, dashboard, width),
      items: bars
        .iter()
        .map(|bar| {
          let value = ctx.t_args(
            "usage.tokens.count",
            [("count", &*format::exact(ctx, measure.of(bar.total())))],
          );
          (bar_name(ctx, bar), value)
        })
        .collect(),
      hovered,
      style: ChartStyle::of(ctx),
    };
    if let Some(canvas) = self.reference.as_canvas().filter(|canvas| canvas.is_attached()) {
      draw(&canvas, &plot);
    }
    *self.plot.lock() = Some(plot);
    let count = bars.len();
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let point = move |event: MouseEvent| {
      let Some(canvas) = reference.as_canvas() else {
        return;
      };
      let index = canvas
        .point_from_window(event.x, event.y)
        .and_then(|(x, _)| Frame::of(&canvas).slot_at(x, count));
      hover(&canvas, &shared, &signal, index);
    };
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let leave = move || {
      if let Some(canvas) = reference.as_canvas() {
        hover(&canvas, &shared, &signal, None);
      }
    };
    let canvas = Canvas::new()
      .ref_element(self.reference.clone())
      .id("usage-tokens-chart")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOKENS_PLOT_HEIGHT))
      .on_mouse_move(point)
      .on_mouse_leave(leave);
    let tip = hovered.and_then(|index| Some((index, bars.get(index)?)));
    Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOKENS_PLOT_HEIGHT))
      .child(canvas)
      .with_children(tip.map(|(index, bar)| {
        let x = tooltip_x(width.unwrap_or(0.0), index, count);
        Element::from(tooltip(ctx, dashboard, bar).absolute_position(x, 0.0))
      }))
      .into()
  }
}

/// Marks bar `index` hovered, or none, and redraws at once; the tooltip follows the signal.
fn hover(
  canvas: &lurq::canvas::CanvasHandle,
  shared: &Mutex<Option<TokenPlot>>,
  signal: &Signal<Hovered>,
  index: Option<usize>,
) {
  if signal.get_untracked().0 == index {
    return;
  }
  if let Some(plot) = shared.lock().as_mut() {
    plot.hovered = index;
    draw(canvas, plot);
  }
  signal.set(Hovered(index));
}

/// Beside the hovered bar, on its left when the right has no room.
fn tooltip_x(width: f32, index: usize, count: usize) -> f32 {
  let frame = Frame {
    left: crate::theme::chart::AXIS_GUTTER,
    top: 0.0,
    right: width,
    bottom: 0.0,
  };
  let (slot, slot_width) = frame.slot(index, count);
  let right = slot + slot_width + TOOLTIP_OFFSET;
  if right + TOOLTIP_WIDTH <= width {
    right
  } else {
    (slot - TOOLTIP_OFFSET - TOOLTIP_WIDTH).max(0.0)
  }
}

/// A bar's name: its day (`Mon 29`) or its hour (`Mon 29, 14:00`).
fn bar_name(ctx: &Ctx, bar: &SeriesBar) -> Arc<str> {
  let day = format::weekday(ctx, bar.bucket.date);
  match bar.bucket.hour {
    Some(hour) => ctx.t_args(
      "usage.date.dateTime",
      [("day", &*day), ("time", format::hour(hour).as_str())],
    ),
    None => day,
  }
}

/// The x axis labels: days, *Today* for today's bar, or hours of one day; every few where many.
fn labels(ctx: &Ctx, dashboard: &Dashboard, width: Option<f32>) -> Vec<Option<Arc<str>>> {
  let bars = &dashboard.report.series;
  let today = local_date(dashboard.now);
  let plot_width = width.map_or(f32::MAX, |width| width - crate::theme::chart::AXIS_GUTTER);
  let days = dashboard.range.days();
  bars
    .iter()
    .enumerate()
    .map(|(index, bar)| match (dashboard.grouping, bar.bucket.hour) {
      (Grouping::Hour, Some(hour)) if days == 1 => (hour % 3 == 0).then(|| format::hour(hour).into()),
      (Grouping::Hour, Some(hour)) => (hour == 0).then(|| format::weekday(ctx, bar.bucket.date)),
      _ if bar.bucket.date == today => Some(ctx.t("usage.tokens.today")),
      _ => {
        (index % label_every(bars.len(), plot_width, LABEL_ROOM) == 0).then(|| format::weekday(ctx, bar.bucket.date))
      }
    })
    .collect()
}

/// The hovered bar's tooltip (`BLY4R`): `$raised`, `$border`, radius 8, the popover shadow.
fn tooltip(ctx: &Ctx, dashboard: &Dashboard, bar: &SeriesBar) -> Column {
  let line = |name: Element, value: Arc<str>| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(name)
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&value)
          .variant(crate::theme::ExtraTypography::MonoValue.role())
          .nowrap(),
      )
  };
  let caption = |text: Arc<str>| Text::new(&text).variant(TypographyStyle::Caption).nowrap().into();
  let clis = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .map(|series| {
      let name = Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(swatch(series_color(series)))
        .child(caption(format::series_name(ctx, series)));
      line(name.into(), format::exact(ctx, bar.by_cli[series.index()].counted()))
    });
  Column::new()
    .id("usage-tokens-tooltip")
    .width(Dimension::Px(TOOLTIP_WIDTH))
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(ExtraRadius::Notice.role())
    .box_shadow(ExtraShadow::Popover.role())
    .child(Text::new(&bar_name(ctx, bar)).variant(TypographyStyle::Label))
    .with_children(clis)
    .child(line(
      caption(ctx.t("usage.measure.spent")),
      format::exact(ctx, bar.total().counted()),
    ))
    .child(line(caption(ctx.t("usage.summary.runs")), bar.runs.to_string().into()))
}

/// Whether a CLI's series has anything in the range.
fn shown(dashboard: &Dashboard, series: CliSeries) -> bool {
  dashboard
    .report
    .series
    .iter()
    .any(|bar| bar.by_cli[series.index()] != Default::default())
}

/// The legend: each CLI with its total for the measure.
fn totals(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let items = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .map(|series| {
      let total: u64 = dashboard
        .report
        .series
        .iter()
        .map(|bar| measure.of(bar.by_cli[series.index()]))
        .sum();
      legend_item(
        series_color(series),
        &format::series_name(ctx, series),
        &format::tokens(total),
        false,
      )
    })
    .collect();
  legend(items, None)
}

/// The bars as a table with exact numbers (`CDcij`).
fn tokens_table(ctx: &Ctx, dashboard: &Dashboard, measure: Measure) -> Element {
  let series: Vec<CliSeries> = CliSeries::ALL
    .into_iter()
    .filter(|series| shown(dashboard, *series))
    .collect();
  let measure_key = match measure {
    Measure::Spent => "usage.measure.spent",
    Measure::Cached => "usage.measure.cached",
    Measure::Output => "usage.measure.output",
  };
  let mut columns: Vec<TableColumn> = vec![column(ctx.t("usage.table.when"), Width::Flex(1.0), false)];
  columns.extend(
    series
      .iter()
      .map(|series| column(format::series_name(ctx, *series), Width::Flex(1.0), true)),
  );
  columns.push(column(ctx.t(measure_key), Width::Flex(1.0), true));
  columns.push(column(ctx.t("usage.summary.runs"), Width::Flex(0.6), true));
  let hourly = dashboard.grouping == Grouping::Hour;
  let rows = dashboard
    .report
    .series
    .iter()
    .filter(|bar| !hourly || bar.total() != Default::default())
    .map(|bar| {
      let mut cells = vec![Cell::Text(bar_name(ctx, bar))];
      cells.extend(
        series
          .iter()
          .map(|series| Cell::Text(format::exact(ctx, measure.of(bar.by_cli[series.index()])))),
      );
      cells.push(Cell::Text(format::exact(ctx, measure.of(bar.total()))));
      cells.push(Cell::Text(bar.runs.to_string().into()));
      row(&columns, cells)
    })
    .collect();
  table("usage-tokens-table", &columns, rows)
}
