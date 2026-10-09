//! Stopped at a limit (`Task · Summary · stopped at token limit` `oTzJZ`): under where the task stands and
//! the run's stages, *Raise the limit* ([`card`]): what the run used of the limit that stopped it, a field
//! for the new limit in whole numbers with what it reads as, what continuing does, *Continue with …*, and
//! *Remove the limit*; then the tokens each step used ([`by_step`]). The new limit applies to this run
//! only. Limits have no maximum (the person's decision of 2026-10-09): any whole number above what was used
//! is taken, and a runner that still holds a project maximum says so when it refuses.
//!
//! Today's run limits are never empty, so *Remove the limit* raises the limit that stopped the run to
//! the largest one, which the app shows as no limit (`budget::unbounded`); once run limits are optional in
//! core (task LIM) it sends no limit instead. A stop at a task group's budget is not lifted by the run's
//! own limits: it says why, and offers nothing to press.

mod by_step;
mod card;

use std::time::Duration;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{
  run::{PauseReason, RaisedLimits, RunLimit, RunStatus},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed, grouped},
};
use orchester_workspace::{
  budget::{minutes_text, unbounded, whole_minutes},
  messages::Message,
  tasks::{
    detail::{RunView, StepView},
    raise::{self, Raise},
  },
};

use self::card::RaiseCard;
use super::{TaskContext, action::decision_action, arranged, stages::stages, task_notice, this_run_panel};
use crate::services::Services;

/// A suggested budget is the current one and half again, rounded up to this.
const SUGGESTION_STEP: u64 = 100_000;
const MINUTE: Duration = Duration::from_secs(60);

/// Which button asked the runner, for the one that shows it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raising {
  Continue,
  Remove,
}

impl lurq::app::component::DevtoolsInspectable for Raising {}

/// The run as it stopped: the limit, what was used of it, and where.
pub struct Stop<'a> {
  pub run: &'a RunView,
  pub limit: RunLimit,
  /// Tokens used of, and the budget of, the run; after a step's budget, of that step's execution.
  /// `None` for a group's budget, which the run does not raise.
  pub tokens: Option<(TokensUsed, TokenBudget)>,
  pub active: Duration,
  pub stopped: Option<&'a StepView>,
}

impl<'a> Stop<'a> {
  fn of(run: &'a RunView, limit: RunLimit, now: Timestamp) -> Self {
    Self {
      run,
      limit,
      tokens: raise::budget_use(run, limit),
      active: run.active_time(now),
      stopped: run.stopped_step(),
    }
  }

  /// What the field takes: minutes after a time limit, tokens after a budget; nothing after a group's.
  pub fn field(&self) -> Option<Unit> {
    match self.limit {
      RunLimit::Time => Some(Unit::Minutes),
      RunLimit::GroupBudget | RunLimit::DailyBudget => None,
      RunLimit::Tokens | RunLimit::StepBudget { .. } => self.tokens.map(|_| Unit::Tokens),
    }
  }

  /// The value offered to continue with: half again the limit that stopped the run.
  fn suggestion(&self, separator: &str) -> String {
    match self.limit {
      RunLimit::Time => {
        let limit = self.run.run.max_duration() + self.run.run.max_duration() / 2;
        let minutes = whole_minutes(limit).max(whole_minutes(self.active) + 1);
        minutes_text(MINUTE * u32::try_from(minutes).unwrap_or(u32::MAX))
      }
      _ => {
        let Some((_, budget)) = self.tokens else {
          return String::new();
        };
        let raised = budget.get().saturating_add(budget.get() / 2);
        grouped(
          raised.div_ceil(SUGGESTION_STEP).saturating_mul(SUGGESTION_STEP),
          separator,
        )
      }
    }
  }

  /// The new limit in `text`, or why it is not one: above what was used, with no maximum. `None` when
  /// the run has nothing to raise (a group's budget).
  pub fn read(&self, text: &str, separator: &str) -> Option<Result<Raise, Message>> {
    let run = &self.run.run;
    Some(match (self.field()?, self.limit, self.tokens) {
      (Unit::Minutes, ..) => raise::time_limit(text, self.active).map(|max_duration| {
        Raise::Limits(RaisedLimits {
          budget: run.budget(),
          max_duration,
        })
      }),
      (Unit::Tokens, _, None) => return None,
      (Unit::Tokens, RunLimit::StepBudget { .. }, Some((used, _))) => {
        let step = self.stopped.map(|step| &*step.name).unwrap_or_default();
        raise::step_budget(text, separator, (used, unbounded()), step).map(Raise::StepBudget)
      }
      (Unit::Tokens, _, Some((used, _))) => raise::run_budget(text, separator, (used, unbounded())).map(|budget| {
        Raise::Limits(RaisedLimits {
          budget,
          max_duration: run.max_duration(),
        })
      }),
    })
  }

  /// No limit in place of the one that stopped the run.
  pub fn removed(&self) -> Raise {
    let run = &self.run.run;
    match self.limit {
      RunLimit::Time => Raise::Limits(RaisedLimits {
        budget: run.budget(),
        max_duration: Duration::MAX,
      }),
      RunLimit::StepBudget { .. } => Raise::StepBudget(unbounded()),
      _ => Raise::Limits(RaisedLimits {
        budget: unbounded(),
        max_duration: run.max_duration(),
      }),
    }
  }
}

/// What the field holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
  Tokens,
  Minutes,
}

pub struct RaiseLimit {
  text: Signal<String>,
  seeded: Signal<bool>,
  asked: Signal<Option<Raising>>,
}

impl Component for RaiseLimit {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      text: ctx.signal(String::new()),
      seeded: ctx.signal(false),
      asked: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = decision_action::<Raising>(ctx, props.project.open.clone());
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    let RunStatus::Paused(PauseReason::LimitReached(limit)) = run.run.status() else {
      return Column::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let stop = Stop::of(run, *limit, now);
    let separator = ctx.t("decisions.groupSeparator");
    if !self.seeded.get_untracked() {
      self.seeded.set(true);
      self.text.set(stop.suggestion(&separator));
    }
    let focus = stop.stopped.map(|step| step.key.as_str().to_owned());
    let raise = RaiseCard {
      stop: &stop,
      text: &self.text,
      asked: &self.asked,
      action: &action,
      separator: &separator,
    }
    .render(ctx);
    let parts: Vec<Element> = vec![
      task_notice(ctx, &props).into(),
      stages(run, focus.as_deref(), now),
      raise,
    ]
    .into_iter()
    .chain(by_step::section(ctx, &stop, now))
    .collect();
    let side = this_run_panel(ctx, &props, None);
    arranged(parts, None, side)
  }
}
