//! The *Plan limits* chart (`C2pTYF`, `e5PbG`): the lines of the windows the range shows, with a
//! tooltip on a hovered point (the window, report time, % used, reset time and the run that
//! reported it) or reset marker (the window and time).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  canvas::CanvasHandle,
  components::{Canvas, Column, Stack, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::{
  Grouping,
  limits::{LimitLine, WindowKind},
};
use parking_lot::Mutex;

use super::{
  format,
  limits_draw::{DrawnLine, DrawnPoint, LimitPlot, draw},
  plot::{Frame, Redraw},
  state::Dashboard,
};
use crate::theme::{
  ExtraRadius, ExtraShadow, ExtraSpacing,
  chart::{CHART_HEIGHT, ChartStyle, TOOLTIP_OFFSET, TOOLTIP_WIDTH},
};

/// Hours between the labels of one day, and the most day labels the axis keeps.
const HOUR_LABEL_STEP: usize = 4;
const DAY_LABELS: usize = 8;

#[derive(Clone, PartialEq)]
pub struct LimitsProps {
  pub dashboard: Arc<Dashboard>,
}

impl lurq::app::component::DevtoolsInspectable for LimitsProps {}

/// What the pointer is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Hovered {
  #[default]
  Nothing,
  Point(usize, usize),
  Reset(usize),
}

impl lurq::app::component::DevtoolsInspectable for Hovered {}

pub struct LimitsChart {
  reference: ElementRef,
  hovered: Signal<Hovered>,
  plot: Arc<Mutex<Option<LimitPlot>>>,
  redraw: Redraw,
}

/// The lines a range shows: by day, 5-hour windows would be a comb of spikes and are left out.
pub fn drawn(dashboard: &Dashboard) -> Vec<&LimitLine> {
  dashboard
    .limits
    .lines
    .iter()
    .filter(|line| dashboard.grouping == Grouping::Hour || line.kind() != WindowKind::FiveHour)
    .collect()
}

/// A window's name: `weekly`, `5-hour`, or its length.
pub fn window_name(ctx: &Ctx, minutes: u64) -> Arc<str> {
  match WindowKind::of(minutes) {
    WindowKind::Weekly => ctx.t("usage.limits.weekly"),
    WindowKind::FiveHour => ctx.t("usage.limits.fiveHour"),
    WindowKind::Other => ctx.t_args("usage.limits.minutes", [("minutes", minutes.to_string().as_str())]),
  }
}

impl Component for LimitsChart {
  type Props = LimitsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      reference: ctx.element_ref(),
      hovered: ctx.signal(Hovered::default()),
      plot: Arc::new(Mutex::new(None)),
      redraw: Redraw::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let dashboard = Arc::clone(&ctx.props::<LimitsProps>().dashboard);
    let hovered = self.hovered.get();
    let plot = plot(ctx, &dashboard, hovered);
    if let Some(canvas) = self.reference.as_canvas().filter(CanvasHandle::is_attached) {
      draw(&canvas, &plot);
    }
    let width = self.reference.as_canvas().map_or(0.0, |canvas| canvas.size().width);
    let tip = tooltip(ctx, &dashboard, &plot, hovered).map(|(at, tip)| {
      let frame = Frame {
        left: crate::theme::chart::AXIS_GUTTER,
        top: 0.0,
        right: width,
        bottom: 0.0,
      };
      let x = plot.x(frame, at) + TOOLTIP_OFFSET;
      let x = if x + TOOLTIP_WIDTH <= width {
        x
      } else {
        (x - 2.0 * TOOLTIP_OFFSET - TOOLTIP_WIDTH).max(0.0)
      };
      Element::from(tip.absolute_position(x, 0.0))
    });
    *self.plot.lock() = Some(plot);
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let point = move |event: MouseEvent| {
      let Some(canvas) = reference.as_canvas() else {
        return;
      };
      let item = canvas
        .point_from_window(event.x, event.y)
        .and_then(|(x, y)| canvas.item_at(x, y));
      hover(
        &canvas,
        &shared,
        &signal,
        item.map_or(Hovered::Nothing, |item| parse(&item.id)),
      );
    };
    let (reference, signal, shared) = (self.reference.clone(), self.hovered.clone(), Arc::clone(&self.plot));
    let leave = move || {
      if let Some(canvas) = reference.as_canvas() {
        hover(&canvas, &shared, &signal, Hovered::Nothing);
      }
    };
    Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CHART_HEIGHT))
      .child(
        Canvas::new()
          .ref_element(self.reference.clone())
          .id("usage-limits-chart")
          .width(Dimension::Pct(100.0))
          .height(Dimension::Px(CHART_HEIGHT))
          .on_mouse_move(point)
          .on_mouse_leave(leave),
      )
      .with_children(tip)
  }

  fn after_layout(&self) {
    if let Some(canvas) = self.reference.as_canvas() {
      self.redraw.bind(&canvas, &self.plot, draw);
    }
  }
}

/// The canvas item `id` stands for (`limit-<line>-<point>`, `reset-<index>`).
fn parse(id: &str) -> Hovered {
  let mut parts = id.split('-');
  match (
    parts.next(),
    parts.next().and_then(|part| part.parse().ok()),
    parts.next().and_then(|part| part.parse().ok()),
  ) {
    (Some("limit"), Some(line), Some(point)) => Hovered::Point(line, point),
    (Some("reset"), Some(index), None) => Hovered::Reset(index),
    _ => Hovered::Nothing,
  }
}

fn hover(canvas: &CanvasHandle, shared: &Mutex<Option<LimitPlot>>, signal: &Signal<Hovered>, hovered: Hovered) {
  if signal.get_untracked() == hovered {
    return;
  }
  if let Some(plot) = shared.lock().as_mut() {
    plot.hovered = match hovered {
      Hovered::Point(line, point) => Some((line, point)),
      Hovered::Reset(_) | Hovered::Nothing => None,
    };
    draw(canvas, plot);
  }
  signal.set(hovered);
}

fn plot(ctx: &Ctx, dashboard: &Dashboard, hovered: Hovered) -> LimitPlot {
  let (start, end) = (dashboard.range.start(), dashboard.range.end());
  let shown_until = dashboard.now.min(end);
  let lines: Vec<DrawnLine> = drawn(dashboard)
    .into_iter()
    .map(|line| DrawnLine {
      series: line.series().index(),
      dashed: line.kind() == WindowKind::FiveHour,
      points: line
        .points
        .iter()
        .map(|point| DrawnPoint {
          at: point.at.unix_millis(),
          percent: point.percent,
          name: ctx.t_args(
            "usage.limits.pointName",
            [
              ("cli", &*format::cli_name(ctx, line.provider)),
              ("window", &*window_name(ctx, line.minutes)),
              ("time", &*format::date_time(ctx, point.at)),
            ],
          ),
          value: format::share(ctx, Some(point.percent.round() as u64)),
        })
        .collect(),
      joins: line.joins.clone(),
    })
    .collect();
  let resets = drawn(dashboard)
    .into_iter()
    .filter(|line| line.kind() == WindowKind::Weekly)
    .flat_map(|line| line.resets.iter().map(move |reset| (line, *reset)))
    .filter(|(_, reset)| start <= *reset && *reset <= shown_until)
    .map(|(line, reset)| (reset.unix_millis(), reset_name(ctx, line, reset)))
    .collect();
  let hatches = dashboard
    .limits
    .first
    .iter()
    .filter(|(_, first)| *first > start)
    .map(|(_, first)| (start.unix_millis(), first.min(&end).unix_millis()))
    .collect();
  let percent = |value: u64| format::share(ctx, Some(value));
  LimitPlot {
    start: start.unix_millis(),
    end: end.unix_millis(),
    lines,
    resets,
    hatches,
    threshold: dashboard.threshold.percent(),
    threshold_label: percent(u64::from(dashboard.threshold.percent())),
    percents: [percent(0), percent(25), percent(50), percent(75), percent(100)],
    labels: labels(ctx, dashboard),
    hovered: match hovered {
      Hovered::Point(line, point) => Some((line, point)),
      _ => None,
    },
    style: ChartStyle::of(ctx),
  }
}

fn reset_name(ctx: &Ctx, line: &LimitLine, at: orchester_core::time::Timestamp) -> Arc<str> {
  ctx.t_args(
    "usage.limits.resetName",
    [
      ("cli", &*format::cli_name(ctx, line.provider)),
      ("window", &*window_name(ctx, line.minutes)),
      ("time", &*format::date_time(ctx, at)),
    ],
  )
}

/// The x axis: one day's hours at every fourth hour from 00:00 to 24:00 (`e5PbG`), or each day at
/// its middle, whatever the grouping, so a week by hour still reads by day.
fn labels(ctx: &Ctx, dashboard: &Dashboard) -> Vec<(i64, Arc<str>)> {
  let (start, end) = (dashboard.range.start(), dashboard.range.end());
  if dashboard.range.days() == 1 {
    let hour = (end.unix_millis() - start.unix_millis()) / 24;
    return (0..=24)
      .step_by(HOUR_LABEL_STEP)
      .map(|index| {
        let at = start.unix_millis() + hour * i64::from(index);
        (at, format::hour(index).into())
      })
      .collect();
  }
  let days = dashboard.range.buckets(Grouping::Day);
  let step = days.len().div_ceil(DAY_LABELS).max(1);
  days
    .windows(2)
    .map(|pair| (pair[0], pair[1].start))
    .chain(days.last().map(|last| (*last, end)))
    .enumerate()
    .filter(|(index, _)| index % step == 0)
    .map(|(_, (day, next))| {
      let middle = (day.start.unix_millis() + next.unix_millis()) / 2;
      let label = if day.date == orchester_core::usage::local_date(dashboard.now) {
        ctx.t("usage.tokens.today")
      } else {
        format::weekday(ctx, day.date)
      };
      (middle, label)
    })
    .collect()
}

/// The hovered point's or reset marker's tooltip, and the time it is at.
fn tooltip(ctx: &Ctx, dashboard: &Dashboard, plot: &LimitPlot, hovered: Hovered) -> Option<(i64, Column)> {
  let lines = drawn(dashboard);
  let (at, rows): (i64, Vec<Arc<str>>) = match hovered {
    Hovered::Nothing => return None,
    Hovered::Reset(index) => {
      let (at, name) = plot.resets.get(index)?;
      (*at, vec![name.clone()])
    }
    Hovered::Point(line, point) => {
      let (line, point) = (lines.get(line)?, lines.get(line)?.points.get(point)?);
      let mut rows = vec![
        ctx.t_args(
          "usage.limits.tooltipWindow",
          [
            ("cli", &*format::cli_name(ctx, line.provider)),
            ("window", &*window_name(ctx, line.minutes)),
          ],
        ),
        ctx.t_args(
          "usage.limits.tooltipReported",
          [("time", &*format::date_time(ctx, point.at))],
        ),
        ctx.t_args(
          "usage.limits.tooltipUsed",
          [("percent", &*format::share(ctx, Some(point.percent.round() as u64)))],
        ),
      ];
      rows.extend(point.resets.map(|resets| {
        ctx.t_args(
          "usage.limits.tooltipResets",
          [("time", &*format::date_time(ctx, resets))],
        )
      }));
      rows.extend(point.source.as_ref().map(|source| {
        ctx.t_args(
          "usage.limits.tooltipRun",
          [
            ("project", &*source.project),
            ("task", &*source.task_key),
            ("run", source.run.get().to_string().as_str()),
          ],
        )
      }));
      (point.at.unix_millis(), rows)
    }
  };
  let mut rows = rows.into_iter();
  let title = rows.next()?;
  Some((
    at,
    Column::new()
      .id("usage-limits-tooltip")
      .width(Dimension::Px(TOOLTIP_WIDTH))
      .padding(SpacingSize::Sm)
      .spacing(ExtraSpacing::Tight.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::Notice.role())
      .box_shadow(ExtraShadow::Popover.role())
      .child(Text::new(&title).variant(TypographyStyle::Label))
      .with_children(rows.map(|row| Text::new(&row).variant(TypographyStyle::Caption))),
  ))
}
