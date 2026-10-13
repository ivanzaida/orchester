//! *Continue this run* (`EwAhH`): the account's windows as its CLI last reported them, the choice between waiting for
//! the account's reset (the default; the runner resumes the same session there by itself) and continuing now on
//! another account of the same CLI as a new session, the other connection when that is chosen, and the action. A
//! reset the CLI did not report leaves nothing to wait for by itself: *Try again now* resumes the same session.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::AgentAccount,
  connection::{AgentConnection, plan_limits::LimitReading},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_workspace::{
  display,
  tasks::{
    detail::RunView,
    plan_limit::{PlanStop, continue_on},
  },
};

use super::{
  super::{
    action::{DecisionAction, state},
    divider,
  },
  Choice, Continuing,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  theme::{DecisionText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::{SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
    option_card::OptionCard,
    plan_window::{PlanWindow, plan_window, share, when},
    unavailable::reason_line,
  },
};

pub(super) const TIMES: (&str, &str) = ("decisions.planLimit.today", "decisions.planLimit.day");
/// A five-hour window and a weekly one, in minutes.
const FIVE_HOURS: u64 = 300;
const WEEK: u64 = 10_080;
/// The other connection's select (`Other connection for 03 Implement` in `EwAhH`).
const SELECT_WIDTH: f32 = 340.0;

/// When the run continues: the time alone today (`16:40`), the day too otherwise.
pub(super) fn reset_time(ctx: &Ctx, resets: Timestamp, now: Timestamp) -> String {
  if resets.local_day_start() == now.local_day_start() {
    orchester_workspace::display::clock(resets)
  } else {
    when(ctx, resets, now, TIMES)
  }
}

/// The account's name: the one given, or *Default*.
pub(super) fn account_name(ctx: &Ctx, account: &AgentAccount) -> String {
  account.name().map_or_else(
    || ctx.t("decisions.planLimit.defaultAccount").to_string(),
    |name| name.as_str().to_owned(),
  )
}

pub(super) struct ChoiceCard<'a> {
  pub run: &'a RunView,
  pub stop: &'a PlanStop,
  pub account: Option<&'a AgentAccount>,
  pub choice: &'a Signal<Choice>,
  pub other: &'a Signal<String>,
  pub asked: &'a Signal<Option<Continuing>>,
  pub action: &'a DecisionAction<Continuing>,
  /// The project's runner is online: continuing needs it.
  pub online: bool,
  /// The window's input gate: continuing on another account is a person's decision.
  pub gate: Option<&'a InputGate>,
}

impl ChoiceCard<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let now = Timestamp::now();
    let resets = self.stop.reading.as_ref().and_then(|reading| reading.resets_at(now));
    if self.stop.others.is_empty() && self.choice.get() == Choice::Other {
      self.choice.set(Choice::Wait);
    }
    let mut card = card("plan-limit-choice", CardSurface::Decision)
      .spacing(ExtraSpacing::Inset.role())
      .child(self.head(ctx));
    if let Some(reading) = &self.stop.reading {
      card = card.child(windows(ctx, reading, now));
    }
    let chosen = self.choice.get();
    card = card.child(divider()).child(self.options(ctx, resets, now));
    if chosen == Choice::Other {
      card = card.child(self.other_select(ctx));
    }
    let (pending, refused) = state(self.action, self.asked.get());
    let offline = (!self.online).then(|| ctx.t("task.controls.offline"));
    card
      .child(self.actions(ctx, resets, now, (pending.is_some(), offline.is_some())))
      .with_children(offline.map(|reason| reason_line("plan-limit-offline", &reason)))
      .with_children(refused.map(|refused| field_error("plan-limit-refused", &message_text(ctx, &refused))))
      .into()
  }

  /// The card's title, and the account it stopped at with who is signed in and the plan.
  fn head(&self, ctx: &Ctx) -> Element {
    let who = self.account.map(|account| {
      let identity = account.last_check().and_then(|check| check.known_identity());
      std::iter::once(account_name(ctx, account))
        .chain(identity.and_then(|identity| identity.email.clone()))
        .chain(
          identity
            .and_then(|identity| identity.plan.as_deref())
            .map(|plan| ctx.t_args("decisions.planLimit.plan", [("plan", plan)]).to_string()),
        )
        .collect::<Vec<_>>()
        .join(" · ")
    });
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Timer, IconSize::Nav, PaletteColor::Warning))
      .child(
        Text::new(&ctx.t("decisions.planLimit.title"))
          .id("plan-limit-title")
          .variant(DecisionText::CardTitle.role()),
      )
      .child(Spacer::new().flex(1.0))
      .with_children(who.map(|who| {
        Text::new(&who)
          .id("plan-limit-account")
          .variant(ExtraTypography::Note.role())
      }))
      .into()
  }

  /// *Wait for …, then continue* beside *Continue now on another account*.
  fn options(&self, ctx: &Ctx, resets: Option<Timestamp>, now: Timestamp) -> Element {
    let account = self
      .account
      .map_or_else(String::new, |account| account_name(ctx, account));
    let agent = self.agent();
    let checkpoint = self
      .run
      .run
      .last_checkpoint()
      .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned())
      .unwrap_or_default();
    let wait_detail = match resets {
      Some(resets) => ctx.t_args(
        "decisions.planLimit.waitDetail",
        [
          ("time", when(ctx, resets, now, TIMES)),
          ("agent", agent.clone()),
          ("account", account.clone()),
        ],
      ),
      None => ctx.t_args(
        "decisions.planLimit.waitDetailNoReset",
        [("agent", agent.clone()), ("account", account.clone())],
      ),
    };
    let (choice, wait_choice) = (self.choice.clone(), self.choice.clone());
    let wait = OptionCard::new(
      "plan-limit-wait",
      (
        Icon::Clock,
        ctx.t_args("decisions.planLimit.wait", [("account", account.as_str())]),
      ),
      wait_detail,
      self.choice.get() == Choice::Wait,
      move || wait_choice.set(Choice::Wait),
    );
    let connection = self
      .chosen_other()
      .map(|other| other.name().as_str().to_owned())
      .unwrap_or_default();
    let other = OptionCard::new(
      "plan-limit-other",
      (Icon::ArrowRightLeft, ctx.t("decisions.planLimit.other")),
      ctx.t_args(
        "decisions.planLimit.otherDetail",
        [
          ("agent", agent),
          ("connection", connection),
          ("checkpoint", checkpoint),
          ("account", account),
        ],
      ),
      self.choice.get() == Choice::Other,
      move || choice.set(Choice::Other),
    );
    let other = if self.stop.others.is_empty() {
      let cli = self.stop.connection.provider().program_name().unwrap_or_default();
      other.unavailable(ctx.t_args("decisions.planLimit.noOther", [("cli", cli)]))
    } else {
      other
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Column::new().flex(1.0).child(wait))
      .child(Column::new().flex(1.0).child(other))
      .into()
  }

  /// The connection to continue on, and what choosing it does.
  fn other_select(&self, ctx: &Ctx) -> Element {
    if self.other.get().is_empty()
      && let Some(first) = self.stop.others.first()
    {
      self.other.set(first.id().to_string());
    }
    // "03 Implement", as the run's stages name it.
    let step = self
      .run
      .stopped_step()
      .map(|step| super::super::step_label(ctx, step).to_string())
      .unwrap_or_default();
    let options = self
      .stop
      .others
      .iter()
      .map(|other| SelectOption::new(other.id().to_string(), other.name().as_str()))
      .collect();
    let label = ctx.t_args("decisions.planLimit.otherConnection", [("step", step.as_str())]);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::End)
      .child(
        Column::new()
          .spacing(SpacingSize::Xs)
          .child(
            Text::new(&label)
              .variant(ExtraTypography::Note.role())
              .color(PaletteColor::TextSecondary),
          )
          .child(
            SelectField::new("plan-limit-connection", label.clone(), self.other.clone(), options)
              .width(Dimension::Px(SELECT_WIDTH))
              .render(ctx),
          ),
      )
      .child(
        Text::new(&ctx.t("decisions.planLimit.otherNote"))
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
      .into()
  }

  /// *Continue at …* while waiting (the runner continues by itself; *Try again now* when the CLI did not say when),
  /// or *Continue now on …*.
  fn actions(&self, ctx: &Ctx, resets: Option<Timestamp>, now: Timestamp, (pending, offline): (bool, bool)) -> Element {
    let run = self.run.id();
    let (asked, action) = (self.asked.clone(), self.action.clone());
    let send = move |command: Command| {
      asked.set(Some(Continuing));
      action.run((Continuing, command));
    };
    let button = match (self.choice.get(), resets) {
      // The person's confirmation of waiting: the runner keeps the run's reset timer and continues it then.
      (Choice::Wait, Some(resets)) => ActionButton::new(
        "plan-limit-continue",
        ButtonKind::Primary,
        ctx.t_args(
          "decisions.planLimit.continueAt",
          [("time", reset_time(ctx, resets, now))],
        ),
      )
      .icon(Icon::Clock)
      .on_click(move || send(Command::WaitForPlanReset { run })),
      (Choice::Wait, None) => ActionButton::new(
        "plan-limit-continue",
        ButtonKind::Primary,
        ctx.t("decisions.planLimit.tryNow"),
      )
      .icon(Icon::RotateCcw)
      .on_click(move || send(Command::ResumeRun { run })),
      (Choice::Other, _) => {
        let chosen = self.chosen_other().cloned();
        let name = chosen
          .as_ref()
          .map(|other| other.name().as_str().to_owned())
          .unwrap_or_default();
        let button = ActionButton::new(
          "plan-limit-continue",
          ButtonKind::Primary,
          ctx.t_args("decisions.planLimit.continueNow", [("connection", name.as_str())]),
        )
        .icon(Icon::ArrowRight);
        let gate = self.gate.cloned();
        match chosen {
          // Which account the step's agent runs on is a person's decision, as the Account field's.
          Some(chosen) => button.on_click(move || {
            if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
              send(continue_on(run, chosen.id()));
            } else {
              tracing::warn!("synthetic input continued a run on another account; not taken");
            }
          }),
          None => button.disabled(),
        }
      }
    };
    let button = if pending {
      button.loading()
    } else if offline {
      button.disabled()
    } else {
      button
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(button)
      .child(
        Text::new(&self.wait_note(ctx, resets, now))
          .id("plan-limit-note")
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
      .into()
  }

  /// What waiting means, or that the runner took the person's *Continue at …*.
  fn wait_note(&self, ctx: &Ctx, resets: Option<Timestamp>, now: Timestamp) -> String {
    let confirmed = self.choice.get() == Choice::Wait && self.action.state().get().is_fulfilled();
    match resets.filter(|_| confirmed) {
      Some(resets) => ctx
        .t_args(
          "decisions.planLimit.waitConfirmed",
          [("time", reset_time(ctx, resets, now))],
        )
        .to_string(),
      None => ctx.t("decisions.planLimit.waitNote").to_string(),
    }
  }

  fn chosen_other(&self) -> Option<&AgentConnection> {
    let chosen = self.other.get();
    self
      .stop
      .others
      .iter()
      .find(|other| other.id().to_string() == chosen)
      .or_else(|| self.stop.others.first())
  }

  fn agent(&self) -> String {
    self
      .run
      .stopped_step()
      .and_then(|step| step.agent.as_deref())
      .unwrap_or_default()
      .to_owned()
  }
}

/// The account's windows, as its CLI last reported them.
fn windows(ctx: &Ctx, reading: &LimitReading, now: Timestamp) -> Element {
  let mut rows = Column::new().id("plan-limit-windows").width(Dimension::Pct(100.0));
  for window in reading.limits.windows.iter().filter(|window| !window.expired(now)) {
    let name = match window.minutes() {
      FIVE_HOURS => ctx.t("decisions.planLimit.fiveHour").to_string(),
      WEEK => ctx.t("decisions.planLimit.weekly").to_string(),
      minutes => ctx
        .t_args("decisions.planLimit.window", [("hours", (minutes / 60).to_string())])
        .to_string(),
    };
    let (used, reached) = share(window, reading, now);
    let resets = window.resets().map_or_else(String::new, |resets| {
      let key = if reached {
        "decisions.planLimit.reached"
      } else {
        "decisions.planLimit.resets"
      };
      ctx.t_args(key, [("time", when(ctx, resets, now, TIMES))]).to_string()
    });
    rows = rows.child(plan_window(
      &format!("plan-limit-window-{}", window.minutes()),
      &PlanWindow {
        name: &name,
        used,
        reached,
        resets: &resets,
      },
    ));
  }
  rows.into()
}
