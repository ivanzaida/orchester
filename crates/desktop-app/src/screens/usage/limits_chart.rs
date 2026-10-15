//! The *Plan limits* chart (`Chart` `yZoLJ`): the lines of the windows the range shows, with a tooltip on a
//! hovered point (`Today, by hour · … hover a point` in `p732q`: the window, % used, when it was reported,
//! when it resets and the run that reported it) or reset marker (the window and when it reset).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
  },
  canvas::CanvasHandle,
  components::{Canvas, Stack},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  time::Timestamp,
  usage::{
    Grouping,
    limits::{LimitLine, WindowKind},
  },
};
use orchester_workspace::usage::Dashboard;
use parking_lot::Mutex;

use super::{
  card::series_color,
  limits_draw::{DrawnLine, DrawnPoint, LimitPlot, RIGHT_ROOM, draw},
  plot::{AXIS_GUTTER, AxisName, AxisText, CHART_HEIGHT, Frame, Redraw},
  tooltip, words,
};
use crate::{
  theme::{
    IconSize,
    usage::{ChartStyle, canvas_font},
  },
  ui::icon::Icon,
};

/// Hours between the labels of one day, and the most day labels the axis keeps.
const HOUR_LABEL_STEP: usize = 4;
const DAY_LABELS: usize = 8;

#[derive(Clone, PartialEq)]
pub struct LimitsProps {
  pub dashboard: Arc<Dashboard>,
}

impl DevtoolsInspectable for LimitsProps {}

/// What the pointer is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Hovered {
  #[default]
  Nothing,
  Point(usize, usize),
  Reset(usize),
}

impl DevtoolsInspectable for Hovered {}

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

/// *Codex · weekly*.
pub fn line_name(ctx: &Ctx, cli: orchester_core::connection::AgentProvider, minutes: u64) -> Arc<str> {
  ctx.t_args(
    "usage.limits.legendName",
    [
      ("cli", &*words::cli_name(ctx, cli)),
      ("window", &*window_name(ctx, minutes)),
    ],
  )
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
    let tip = tip(ctx, &dashboard, &plot, hovered).map(|(at, tip)| {
      let frame = Frame {
        left: AXIS_GUTTER,
        top: 0.0,
        right: width - RIGHT_ROOM,
        bottom: 0.0,
        edge: width,
      };
      let x = plot.x(frame, at);
      tooltip::beside(tip, (x, x), width)
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
  let percent = |value: u64| words::share(ctx, Some(value));
  let lines = drawn(dashboard).into_iter().map(|line| drawn_line(ctx, line)).collect();
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
    .map(|(cli, first)| {
      let label = ctx.t_args("usage.limits.noData", [("cli", &*words::cli_name(ctx, *cli))]);
      (start.unix_millis(), first.min(&end).unix_millis(), label)
    })
    .collect();
  let theme = ctx.theme();
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
    label_kind: if dashboard.range.days() == 1 {
      AxisText::Clock
    } else {
      AxisText::Day
    },
    hovered: match hovered {
      Hovered::Point(line, point) => Some((line, point)),
      _ => None,
    },
    marker: (
      Icon::RotateCcw.glyph().to_string().into(),
      canvas_font(&theme.typography_style(IconSize::Sm.role())),
    ),
    style: ChartStyle::of(ctx),
  }
}

fn drawn_line(ctx: &Ctx, line: &LimitLine) -> DrawnLine {
  DrawnLine {
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
            ("window", &*line_name(ctx, line.provider, line.minutes)),
            ("time", &*words::date_time(ctx, point.at)),
          ],
        ),
        value: words::share(ctx, Some(point.percent.round() as u64)),
      })
      .collect(),
    joins: line.joins.clone(),
    label: words::cli_name(ctx, line.provider),
  }
}

fn reset_name(ctx: &Ctx, line: &LimitLine, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "usage.limits.resetName",
    [
      ("cli", &*words::cli_name(ctx, line.provider)),
      ("window", &*window_name(ctx, line.minutes)),
      ("time", &*words::weekday_time(ctx, at)),
    ],
  )
}

/// The x axis: one day's hours at every fourth hour from 00:00 to 24:00, or each day at its middle, so a
/// week by hour still reads by day; every few days where many, counted back from the last day.
fn labels(ctx: &Ctx, dashboard: &Dashboard) -> Vec<(i64, AxisName)> {
  let (start, end) = (dashboard.range.start(), dashboard.range.end());
  if dashboard.range.days() == 1 {
    let hour = (end.unix_millis() - start.unix_millis()) / 24;
    return (0..=24)
      .step_by(HOUR_LABEL_STEP)
      .map(|index| {
        let at = start.unix_millis() + hour * i64::from(index);
        (at, AxisName::plain(words::hour(index).into()))
      })
      .collect();
  }
  let days = dashboard.range.buckets(Grouping::Day);
  let today = dashboard.today();
  let step = days.len().div_ceil(DAY_LABELS).max(1);
  let last = days.len().saturating_sub(1);
  days
    .windows(2)
    .map(|pair| (pair[0], pair[1].start))
    .chain(days.last().map(|last| (*last, end)))
    .enumerate()
    .filter(|(index, _)| (last - index).is_multiple_of(step))
    .map(|(_, (day, next))| {
      let middle = (day.start.unix_millis() + next.unix_millis()) / 2;
      (middle, AxisName::day(ctx, day.date, today))
    })
    .collect()
}

/// The hovered point's or reset marker's tooltip, and the time it is at.
fn tip(ctx: &Ctx, dashboard: &Dashboard, plot: &LimitPlot, hovered: Hovered) -> Option<(i64, Element)> {
  match hovered {
    Hovered::Nothing => None,
    Hovered::Reset(index) => {
      let (at, name) = plot.resets.get(index)?;
      Some((*at, tooltip::tip("usage-limits-tooltip", name, Vec::new(), None)))
    }
    Hovered::Point(line, point) => {
      let line = *drawn(dashboard).get(line)?;
      let point = line.points.get(point)?;
      let used = ctx.t_args(
        "usage.limits.used",
        [("percent", &*words::share(ctx, Some(point.percent.round() as u64)))],
      );
      let mut rows = vec![
        tooltip::line(Some(series_color(line.series())), &used, None),
        tooltip::line(
          None,
          &ctx.t_args(
            "usage.limits.reported",
            [("time", &*words::reported(ctx, point.at, dashboard.now))],
          ),
          None,
        ),
      ];
      rows.extend(point.resets.map(|resets| {
        tooltip::line(
          None,
          &ctx.t_args(
            "usage.limits.resetsAt",
            [("time", &*words::reset_time(ctx, resets, dashboard.now))],
          ),
          None,
        )
      }));
      let source = point.source.as_ref().map(|source| {
        ctx.t_args(
          "usage.limits.from",
          [
            ("project", &*source.project),
            ("run", &*words::run_label(ctx, &source.task_key, source.run.get())),
          ],
        )
      });
      let title = ctx.t_args(
        "usage.limits.windowTitle",
        [("window", &*line_name(ctx, line.provider, line.minutes))],
      );
      Some((
        point.at.unix_millis(),
        tooltip::tip("usage-limits-tooltip", &title, rows, source.as_deref()),
      ))
    }
  }
}
