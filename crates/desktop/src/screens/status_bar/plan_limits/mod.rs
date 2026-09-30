//! Plan limits (`XCXvw`, states M–R in `poPso`): one live account entry per CLI.
mod detail;
mod metadata;
mod text;

use std::time::Duration;

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
    plan_limits::{LimitReading, LimitState, PlanReadings},
  },
  time::Timestamp,
};

use super::{
  POPOVER_GAP,
  item::{glyph, item, label, value},
};
use crate::{theme::ExtraTypography, ui::icon::Icon};

#[derive(Clone, PartialEq, Eq)]
pub struct Props {
  pub project_name: String,
  pub connections: Vec<AgentConnection>,
  pub readings: PlanReadings,
}
impl lurq::app::component::DevtoolsInspectable for Props {}

pub struct PlanLimits {
  hover: Signal<bool>,
  now: Signal<i64>,
  anchor: ElementRef,
}

impl Component for PlanLimits {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let now = ctx.signal(Timestamp::now().unix_millis());
    let tick = now.clone();
    ctx
      .create_interval(Duration::from_secs(1), move || tick.set(Timestamp::now().unix_millis()))
      .start();
    Self {
      hover: ctx.signal(false),
      now,
      anchor: ElementRef::new(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>();
    let entries = entries(props, Timestamp::from_unix_millis(self.now.get()));
    let reached = entries.iter().any(|entry| entry.state() == LimitState::Reached);
    let near = entries.iter().any(|entry| entry.state() == LimitState::Near);
    let icon = if reached {
      Icon::OctagonPause
    } else if near {
      Icon::AlertCircle
    } else {
      Icon::Hourglass
    };
    let enter = self.hover.clone();
    let leave = self.hover.clone();
    let mut bar = item("status-plan-limits")
      .flex_shrink(0.0)
      .ref_element(self.anchor.clone())
      .cursor(CursorIcon::Default)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(glyph(
        icon,
        if near || reached {
          PaletteColor::TextPrimary
        } else {
          PaletteColor::TextSecondary
        },
      ));
    if reached {
      bar = bar.background(PaletteColor::SurfaceRaised);
    }
    for (index, entry) in entries.iter().enumerate() {
      if index > 0 {
        bar = bar.child(label(&ctx.t("statusBar.planLimits.separator"), PaletteColor::TextMuted));
      }
      bar = bar.with_children(compact(ctx, entry));
    }
    let mut result = Column::new().child(bar);
    if self.hover.get() {
      let panel = detail::panel(ctx, &entries);
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

fn compact(ctx: &Ctx, entry: &Entry<'_>) -> Vec<Element> {
  let state = entry.state();
  let color = if state == LimitState::Normal {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextPrimary
  };
  if state == LimitState::Reached {
    return vec![
      Text::new(&ctx.t_args(
        "statusBar.planLimits.reached",
        [("cli", &*text::name(ctx, entry.provider))],
      ))
      .variant(ExtraTypography::StatusLabel.role())
      .nowrap()
      .into(),
      value(
        &text::reset(ctx, entry.window().and_then(|window| window.resets()), entry.now, false),
        PaletteColor::TextPrimary,
      ),
    ];
  }
  let percent = entry.window().map(|window| text::percent(ctx, window.percent()));
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
  fn window(&self) -> Option<&orchester_core::connection::plan_limits::LimitWindow> {
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
      .and_then(|reading| reading.limiting(self.now))
      .map_or(LimitState::Normal, |window| window.state(self.now))
  }
}

fn entries(props: &Props, now: Timestamp) -> Vec<Entry<'_>> {
  AgentProvider::ALL
    .into_iter()
    .filter_map(|provider| {
      let connection = props
        .connections
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

pub fn reached(props: &Props) -> bool {
  entries(props, Timestamp::now())
    .iter()
    .any(|entry| entry.state() == LimitState::Reached)
}
