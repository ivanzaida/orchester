//! The working line, the chat's last row while an agent works (`Working line` `js05b`): the turning 13
//! `loader-circle` and who is working and how long ago the last entry arrived, 11.5 `text_muted`, 6 apart. The
//! time is real, kept current by its own one-second interval, so only this line renders every second.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::display;

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

const TICK: Duration = Duration::from_secs(1);

/// Who works, and when the newest entry arrived (`None` before the first); or what it waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Working {
  /// The agent's role; `None` once its profile is gone.
  pub(in super::super) agent: Option<Arc<str>>,
  pub(in super::super) last: Option<Timestamp>,
  /// The agent waits for the person instead of working.
  pub(in super::super) waits: Option<Wait>,
}

/// What an agent waits for, since its request: an answer (a question, a tool request) or a decision on its
/// plan. Its active time is stopped meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Wait {
  pub(in super::super) since: Timestamp,
  pub(in super::super) plan: bool,
}

impl lurq::app::component::DevtoolsInspectable for Working {}

pub(super) struct WorkingLine {
  tick: Signal<u64>,
}

impl Component for WorkingLine {
  type Props = Working;

  fn create(ctx: &mut Ctx) -> Self {
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    ctx
      .create_interval(TICK, move || next.update(|tick| *tick += 1))
      .start();
    Self { tick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Read so every tick renders the elapsed time again.
    self.tick.get();
    let working = ctx.props::<Working>().clone();
    if let Some(wait) = working.waits {
      return line(Icon::Hand, &waits_text(ctx, working.agent.as_deref(), wait));
    }
    let text = match (working.last, working.agent.as_deref()) {
      (Some(last), agent) => {
        // A clock set back since the entry arrived reads as just now.
        let millis = u64::try_from(Timestamp::now().unix_millis() - last.unix_millis()).unwrap_or_default();
        let elapsed = message_text(ctx, &display::duration(Duration::from_millis(millis)));
        match agent {
          Some(agent) => ctx.t_args("conversation.working", [("agent", agent), ("elapsed", &*elapsed)]),
          None => ctx.t_args("conversation.workingUnnamed", [("elapsed", &*elapsed)]),
        }
      }
      (None, Some(agent)) => ctx.t_args("conversation.workingStarted", [("agent", agent)]),
      (None, None) => ctx.t("conversation.workingStartedUnnamed"),
    };
    line(Icon::LoaderCircle, &text)
  }
}

/// *Builder waits for your answer · since 16:44, its active time stopped* (under `Approval card · waiting` in
/// `YhiGC`).
fn waits_text(ctx: &Ctx, agent: Option<&str>, wait: Wait) -> Arc<str> {
  let since = display::clock(wait.since);
  let key = match (agent.is_some(), wait.plan) {
    (true, false) => "conversation.waitsAnswer",
    (true, true) => "conversation.waitsDecision",
    (false, false) => "conversation.waitsAnswerUnnamed",
    (false, true) => "conversation.waitsDecisionUnnamed",
  };
  ctx.t_args(key, [("agent", agent.unwrap_or_default()), ("since", since.as_str())])
}

/// The line: `glyph` (a loader turns) and `text`.
fn line(glyph: Icon, text: &str) -> Row {
  Row::new()
    .id("conversation-working")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(text).variant(ExtraTypography::Note.role()))
}
