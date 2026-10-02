//! Plan limits (`XCXvw`, states M–R in `poPso`): one live account entry per CLI.
mod detail;
mod metadata;
mod text;

use std::{
  sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
  },
  time::Duration,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, Placement},
    theme::PaletteColor,
  },
  components::{Column, Popup, Text},
  core::{ElementRef, Signal},
  node::{CursorIcon, Element, HitTestBehavior},
};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider,
    plan_limits::{LimitBlock, LimitReading, LimitState, LimitWindow, PlanReadings},
  },
  time::Timestamp,
};

use super::{
  POPOVER_GAP,
  item::{glyph, item, label, value},
};
use crate::{plan_limits::Connections, theme::ExtraTypography, ui::icon::Icon};

/// How often an open hint's ages and countdowns, and a closed item's next reset, are looked at.
const TICK: Duration = Duration::from_secs(1);
const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, PartialEq, Eq)]
pub struct Props {
  pub project_name: String,
  pub connections: Connections,
  pub readings: PlanReadings,
  /// Reached entries leave out their reset or reason, which the hint still gives.
  pub brief: bool,
}
impl lurq::app::component::DevtoolsInspectable for Props {}

pub struct PlanLimits {
  hover: Signal<bool>,
  now: Signal<i64>,
  /// When the closed item next changes on its own (a window resets, or comes within a day of
  /// it); the timer re-renders only then, or every tick while the hint is open.
  next_change: Arc<AtomicI64>,
  anchor: ElementRef,
}

impl Component for PlanLimits {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let hover = ctx.signal(false);
    let now = ctx.signal(Timestamp::now().unix_millis());
    let next_change = Arc::new(AtomicI64::new(i64::MAX));
    let (open, tick, next) = (hover.clone(), now.clone(), next_change.clone());
    ctx
      .create_interval(TICK, move || {
        let at = Timestamp::now().unix_millis();
        if open.get_untracked() || at >= next.load(Ordering::Relaxed) {
          tick.set(at);
        }
      })
      .start();
    Self {
      hover,
      now,
      next_change,
      anchor: ElementRef::new(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>();
    let now = Timestamp::from_unix_millis(self.now.get());
    let entries = entries(props, now);
    self.next_change.store(next_change(&entries, now), Ordering::Relaxed);
    let (bar, panel) = match &props.connections {
      Connections::Failed(error) => (self.unreadable(ctx), detail::unreadable(ctx, error)),
      _ => (self.bar(ctx, &entries, props.brief), detail::panel(ctx, &entries)),
    };
    let mut result = Column::new().child(bar);
    if self.hover.get() {
      result = result.child(
        Popup::new(self.anchor.clone(), panel)
          .open_when(true)
          .placement(Placement::TopStart)
          .offset(0.0, POPOVER_GAP)
          .hit_test(HitTestBehavior::None)
          .dismiss_on_outside_click(false)
          .dismiss_on_escape(false),
      );
    }
    result
  }
}

impl PlanLimits {
  fn frame(&self, glyph_icon: Icon, emphasis: bool) -> lurq::components::Button {
    let (enter, leave, now) = (self.hover.clone(), self.hover.clone(), self.now.clone());
    item("status-plan-limits")
      .flex_shrink(0.0)
      .ref_element(self.anchor.clone())
      .cursor(CursorIcon::Default)
      .on_mouse_enter(move || {
        now.set(Timestamp::now().unix_millis());
        enter.set(true);
      })
      .on_mouse_leave(move || leave.set(false))
      .child(glyph(
        glyph_icon,
        if emphasis {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        },
      ))
  }

  fn bar(&self, ctx: &Ctx, entries: &[Entry<'_>], brief: bool) -> Element {
    let reached = entries.iter().any(|entry| entry.state() == LimitState::Reached);
    let near = entries.iter().any(|entry| entry.state() == LimitState::Near);
    let icon = if reached {
      Icon::OctagonPause
    } else if near {
      Icon::AlertCircle
    } else {
      Icon::Hourglass
    };
    let mut bar = self.frame(icon, near || reached);
    if reached {
      bar = bar.background(PaletteColor::SurfaceRaised);
    }
    for (index, entry) in entries.iter().enumerate() {
      if index > 0 {
        bar = bar.child(label(&ctx.t("statusBar.planLimits.separator"), PaletteColor::TextMuted));
      }
      bar = bar.with_children(compact(ctx, entry, brief));
    }
    bar.into()
  }

  /// The project's connections could not be read: the item stays, and its hint says why.
  fn unreadable(&self, ctx: &Ctx) -> Element {
    self
      .frame(Icon::Hourglass, false)
      .child(label(&ctx.t("statusBar.planLimits.title"), PaletteColor::TextSecondary))
      .child(value(&ctx.t("statusBar.counts.unknown"), PaletteColor::TextMuted))
      .into()
  }
}

fn compact(ctx: &Ctx, entry: &Entry<'_>, brief: bool) -> Vec<Element> {
  let state = entry.state();
  let color = if state == LimitState::Normal {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextPrimary
  };
  if state == LimitState::Reached {
    // A reason is prose in the label's role; a reset time is a value in mono, as in `WuIp9`.
    let detail = match entry.block_beyond_reset() {
      _ if brief => None,
      Some(LimitBlock::Credits) => Some(label(
        &ctx.t("statusBar.planLimits.reasonCredits"),
        PaletteColor::TextPrimary,
      )),
      Some(LimitBlock::Workspace) => Some(label(
        &ctx.t("statusBar.planLimits.reasonWorkspace"),
        PaletteColor::TextPrimary,
      )),
      Some(_) => None,
      None => Some(value(
        &text::reset(ctx, entry.window().and_then(LimitWindow::resets), entry.now, false),
        PaletteColor::TextPrimary,
      )),
    };
    let heading = Text::new(&ctx.t_args(
      "statusBar.planLimits.reached",
      [("cli", &*text::name(ctx, entry.provider))],
    ))
    .variant(ExtraTypography::StatusLabel.role())
    .nowrap()
    .into();
    return std::iter::once(heading).chain(detail).collect();
  }
  let percent = entry
    .window()
    .and_then(LimitWindow::percent)
    .map(|percent| text::percent(ctx, percent));
  let value_color = if percent.is_some() {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let variant = if state == LimitState::Near {
    ExtraTypography::MonoLimit
  } else {
    ExtraTypography::MonoValue
  };
  vec![
    label(&text::name(ctx, entry.provider), color),
    Text::new(&percent.unwrap_or_else(|| ctx.t("statusBar.counts.unknown")))
      .variant(variant.role())
      .color(value_color)
      .nowrap()
      .into(),
  ]
}

pub(super) struct Entry<'a> {
  provider: AgentProvider,
  project_name: &'a str,
  connection: &'a AgentConnection,
  reading: Option<&'a LimitReading>,
  now: Timestamp,
}

impl Entry<'_> {
  fn window(&self) -> Option<&LimitWindow> {
    let reading = self.reading?;
    if self.state() == LimitState::Reached {
      reading.limiting(self.now)
    } else {
      reading.fullest(self.now)
    }
  }
  fn state(&self) -> LimitState {
    self
      .reading
      .map_or(LimitState::Normal, |reading| reading.state(self.now))
  }
  /// What the CLI refuses turns for, when it is not a window that lifts at its reset.
  fn block_beyond_reset(&self) -> Option<LimitBlock> {
    self.reading?.block_beyond_reset()
  }
}

fn entries(props: &Props, now: Timestamp) -> Vec<Entry<'_>> {
  let Connections::Ready(connections) = &props.connections else {
    return Vec::new();
  };
  AgentProvider::CLI
    .into_iter()
    .filter_map(|provider| {
      let connection = connections
        .iter()
        .filter(|connection| connection.provider() == provider)
        .max_by_key(|connection| {
          (
            connection.status().is_usable(),
            connection.last_check().map(|check| check.at),
          )
        })?;
      let reading = props.readings.get(provider).filter(|reading| {
        !connection
          .last_check()
          .is_some_and(|check| check.at > reading.at && !check.result.status().is_usable())
      });
      Some(Entry {
        provider,
        project_name: &props.project_name,
        connection,
        reading,
        now,
      })
    })
    .collect()
}

/// The next instant after `now` at which a closed item reads differently: a window resets, or
/// its reset comes within a day and is shown by its time instead of its day.
fn next_change(entries: &[Entry<'_>], now: Timestamp) -> i64 {
  let now = now.unix_millis();
  entries
    .iter()
    .filter_map(|entry| entry.reading)
    .flat_map(|reading| &reading.limits.windows)
    .filter_map(LimitWindow::resets)
    .flat_map(|at| [at.unix_millis(), at.unix_millis() - DAY_MILLIS])
    .filter(|&at| at > now)
    .min()
    .unwrap_or(i64::MAX)
}

/// Whether the item has anything to show: an agent CLI connection, or that the connections could
/// not be read. Nothing is shown while they are read.
pub fn shown(props: &Props) -> bool {
  match &props.connections {
    Connections::Loading => false,
    Connections::Ready(connections) => connections
      .iter()
      .any(|connection| AgentProvider::CLI.contains(&connection.provider())),
    Connections::Failed(_) => true,
  }
}

/// How many CLIs are reached or refused, which decides how wide the item is.
pub fn reached_count(props: &Props) -> usize {
  entries(props, Timestamp::now())
    .iter()
    .filter(|entry| entry.state() == LimitState::Reached)
    .count()
}
