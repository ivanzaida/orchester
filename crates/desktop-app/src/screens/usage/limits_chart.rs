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
  connection::AgentProvider,
  id::AgentAccountId,
  time::Timestamp,
  usage::{
    Grouping,
    limits::{LimitLine, WindowKind},
  },
};
use orchester_workspace::usage::{AccountLabel, Dashboard};
use parking_lot::Mutex;

use super::{
  limits_draw::{DrawnLine, DrawnPoint, LimitPlot, RIGHT_ROOM, draw},
  plot::{AXIS_GUTTER, AxisName, AxisText, CHART_HEIGHT, Frame, Redraw},
  series::{account_name, account_text, cli_short, series_swatch},
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

#[derive(Clone)]
pub struct LimitsProps {
  pub dashboard: Arc<Dashboard>,
}

impl PartialEq for LimitsProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.dashboard, &other.dashboard)
  }
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

/// A window's name: `weekly`, `5-hour`, or its length.
pub fn window_name(ctx: &Ctx, minutes: u64) -> Arc<str> {
  match WindowKind::of(minutes) {
    WindowKind::Weekly => ctx.t("usage.limits.weekly"),
    WindowKind::FiveHour => ctx.t("usage.limits.fiveHour"),
    WindowKind::Other => ctx.t_args("usage.limits.minutes", [("minutes", minutes.to_string().as_str())]),
  }
}

/// Whose limits a window is: its account (*Codex · Default*), or its CLI where it has no accounts.
pub fn window_owner(
  ctx: &Ctx,
  dashboard: &Dashboard,
  (provider, account): (AgentProvider, Option<AgentAccountId>),
) -> Arc<str> {
  account.map_or_else(
    || words::cli_name(ctx, provider),
    |account| account_text(ctx, &dashboard.account_label(account)),
  )
}

/// *Codex · Default · weekly*.
pub fn line_name(
  ctx: &Ctx,
  dashboard: &Dashboard,
  (provider, account, minutes): (AgentProvider, Option<AgentAccountId>, u64),
) -> Arc<str> {
  ctx.t_args(
    "usage.limits.legendName",
    [
      ("account", &*window_owner(ctx, dashboard, (provider, account))),
      ("window", &*window_name(ctx, minutes)),
    ],
  )
}

/// A line's direct label beside its last point, on two lines: the CLI over the account (*Codex* / *Default*).
fn direct_label(
  ctx: &Ctx,
  dashboard: &Dashboard,
  (provider, account): (AgentProvider, Option<AgentAccountId>),
) -> (Arc<str>, Option<Arc<str>>) {
  match account.map(|account| dashboard.account_label(account)) {
    Some(AccountLabel::Known { cli, name }) => (cli_short(ctx, cli), Some(account_name(ctx, name.as_ref()))),
    Some(AccountLabel::Removed) | None => (words::cli_name(ctx, provider), None),
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
    let tip = tip(ctx, &dashboard, &plot, hovered).map(|(at, tip)| {
      let frame = Frame {
        left: AXIS_GUTTER,
        top: 0.0,
        right: width - RIGHT_ROOM,
        bottom: 0.0,
        edge: width,
      };
      let x = plot.x(frame, at);
      let tip_width = if matches!(hovered, Hovered::Reset(_)) {
        tooltip::TITLE_WIDTH
      } else {
        tooltip::WIDTH
      };
      tooltip::beside((self.reference.clone(), CHART_HEIGHT), (tip, tip_width), (x, x), width)
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
  let lines = dashboard
    .drawn_limits()
    .into_iter()
    .map(|line| drawn_line(ctx, dashboard, line))
    .collect();
  let resets = dashboard
    .drawn_limits()
    .into_iter()
    .filter(|line| line.kind() == WindowKind::Weekly)
    .flat_map(|line| line.resets.iter().map(move |reset| (line, *reset)))
    .filter(|(_, reset)| start <= *reset && *reset <= shown_until)
    .map(|(line, reset)| (reset.unix_millis(), reset_name(ctx, dashboard, line, reset)))
    .collect();
  let hatches = dashboard
    .limits
    .first_by_account
    .iter()
    .filter(|(_, _, first)| *first > start)
    .map(|(provider, account, first)| {
      let owner = window_owner(ctx, dashboard, (*provider, *account));
      let label = ctx.t_args("usage.limits.noData", [("account", &*owner)]);
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

fn drawn_line(ctx: &Ctx, dashboard: &Dashboard, line: &LimitLine) -> DrawnLine {
  let name = line_name(ctx, dashboard, (line.provider, line.account, line.minutes));
  DrawnLine {
    series: dashboard.account_series(line.account).index(),
    dashed: line.kind() == WindowKind::FiveHour,
    points: line
      .points
      .iter()
      .map(|point| DrawnPoint {
        at: point.at.unix_millis(),
        percent: point.percent,
        name: ctx.t_args(
          "usage.limits.pointName",
          [("window", &*name), ("time", &*words::date_time(ctx, point.at))],
        ),
        value: words::share(ctx, Some(point.percent.round() as u64)),
      })
      .collect(),
    joins: line.joins.clone(),
    label: direct_label(ctx, dashboard, (line.provider, line.account)),
  }
}

fn reset_name(ctx: &Ctx, dashboard: &Dashboard, line: &LimitLine, at: Timestamp) -> Arc<str> {
  ctx.t_args(
    "usage.limits.resetName",
    [
      ("account", &*window_owner(ctx, dashboard, (line.provider, line.account))),
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
      Some((*at, tooltip::title_tip("usage-limits-tooltip", name)))
    }
    Hovered::Point(line, point) => {
      let line = *dashboard.drawn_limits().get(line)?;
      let point = line.points.get(point)?;
      let used = ctx.t_args(
        "usage.limits.used",
        [("percent", &*words::share(ctx, Some(point.percent.round() as u64)))],
      );
      let mut rows = vec![
        tooltip::line(Some(series_swatch(dashboard.account_series(line.account))), &used, None),
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
        [(
          "window",
          &*line_name(ctx, dashboard, (line.provider, line.account, line.minutes)),
        )],
      );
      Some((
        point.at.unix_millis(),
        tooltip::tip("usage-limits-tooltip", &title, rows, source.as_deref()),
      ))
    }
  }
}
