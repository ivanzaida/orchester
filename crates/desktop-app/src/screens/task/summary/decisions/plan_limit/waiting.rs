//! A new run waiting for its account's plan limit (`XPjSc`, Task › Summary): the step's agent did not start, as its
//! account's window is used up; the run holds no agent slot and starts by itself at the reset. *Or start … now on*
//! another account's connection of the same CLI starts the step there, for this run only; the profile keeps its own
//! connection, and later steps wait for their own accounts the same way. Starting elsewhere is a person's decision.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{account::AgentAccount, connection::AgentConnection, time::Timestamp};
use orchester_workspace::tasks::{
  detail::RunView,
  plan_limit::{PlanStop, continue_on, try_now},
};

use super::{
  super::{
    action::{DecisionAction, state},
    step_label,
  },
  Continuing,
  card::{account_name, reset_time},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  services::accounts::Accounts,
  theme::{DecisionText, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::{SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
  },
};

/// The other connection's select.
const SELECT_WIDTH: f32 = 280.0;

pub(super) struct WaitingCard<'a> {
  pub run: &'a RunView,
  pub stop: &'a PlanStop,
  pub account: Option<&'a AgentAccount>,
  pub accounts: &'a Accounts,
  pub other: &'a Signal<String>,
  pub asked: &'a Signal<Option<Continuing>>,
  pub action: &'a DecisionAction<Continuing>,
  pub online: bool,
  pub gate: Option<&'a InputGate>,
}

impl WaitingCard<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let now = Timestamp::now();
    let account = self
      .account
      .map(|account| account_name(ctx, account))
      .unwrap_or_default();
    let step = self
      .run
      .stopped_step()
      .map(|step| step_label(ctx, step).to_string())
      .unwrap_or_default();
    let resets = self.stop.reading.as_ref().and_then(|reading| reading.resets_at(now));
    let title = match resets {
      Some(resets) => ctx.t_args(
        "decisions.planLimit.waitingTitle",
        [("account", account.clone()), ("time", reset_time(ctx, resets, now))],
      ),
      None => ctx.t_args(
        "decisions.planLimit.waitingTitleNoReset",
        [("account", account.clone())],
      ),
    };
    let body = ctx.t_args(
      "decisions.planLimit.waitingBody",
      [
        ("run", self.run.run.number().get().to_string()),
        ("step", step.clone()),
        ("connection", self.stop.connection.name().as_str().to_owned()),
      ],
    );
    let head = card("plan-limit-waiting", CardSurface::Attention)
      .spacing(SpacingSize::Xs)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(icon(Icon::Clock, IconSize::Nav, PaletteColor::TextSecondary))
          .child(
            Text::new(&title)
              .id("plan-limit-waiting-title")
              .variant(DecisionText::CardTitle.role()),
          ),
      )
      .child(
        Text::new(&body)
          .id("plan-limit-waiting-body")
          .variant(ExtraTypography::Prose.role())
          .width(Dimension::Pct(100.0)),
      );
    let try_row = self.try_here(ctx, &account);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(head)
      .child(try_row)
      .child(self.elsewhere(ctx, &step))
      .into()
  }

  /// *Try now on …*: the step starts on its own account once, a person's decision, for a limit lifted early or a
  /// stale reading; if the CLI still refuses, the run waits again.
  fn try_here(&self, ctx: &Ctx, account: &str) -> Element {
    let (pending, _) = state(self.action, self.asked.get());
    let button = ActionButton::new(
      "plan-limit-try-now",
      ButtonKind::Secondary,
      ctx.t_args("decisions.planLimit.tryNowOn", [("account", account)]),
    )
    .icon(Icon::RotateCcw);
    let button = if pending.is_some() {
      button.loading()
    } else if !self.online {
      button.disabled()
    } else {
      let (run, gate) = (self.run.id(), self.gate.cloned());
      let (asked, action) = (self.asked.clone(), self.action.clone());
      button.on_click(move || {
        if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
          asked.set(Some(Continuing));
          action.run((Continuing, try_now(run)));
        } else {
          tracing::warn!("synthetic input tried a waiting run now; not taken");
        }
      })
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(button)
      .child(
        Text::new(&ctx.t_args("decisions.planLimit.tryNowNote", [("account", account)]))
          .variant(ExtraTypography::Note.role())
          .flex(1.0),
      )
      .into()
  }

  /// *Or start … now on*: the other connections, *Start now on …* and what it changes; or why there is none.
  fn elsewhere(&self, ctx: &mut Ctx, step: &str) -> Element {
    let Some(first) = self.stop.others.first() else {
      let cli = self.stop.connection.provider().program_name().unwrap_or_default();
      return reason_line(
        "plan-limit-waiting-none",
        &ctx.t_args("decisions.planLimit.noOther", [("cli", cli)]),
      )
      .into();
    };
    if self.other.get().is_empty() {
      self.other.set(first.id().to_string());
    }
    let chosen = self
      .stop
      .others
      .iter()
      .find(|other| other.id().to_string() == self.other.get())
      .unwrap_or(first)
      .clone();
    let label = ctx.t_args("decisions.planLimit.startOn", [("step", step)]);
    let options = self
      .stop
      .others
      .iter()
      .map(|other| SelectOption::new(other.id().to_string(), other.name().as_str()))
      .collect();
    let (pending, refused) = state(self.action, self.asked.get());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&label)
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::TextSecondary),
      )
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(
            SelectField::new(
              "plan-limit-waiting-connection",
              label.clone(),
              self.other.clone(),
              options,
            )
            .width(Dimension::Px(SELECT_WIDTH))
            .render(ctx),
          )
          .child(self.start_button(ctx, &chosen, pending.is_some())),
      )
      .with_children(refused.map(|refused| field_error("plan-limit-refused", &message_text(ctx, &refused))))
      .child(
        Text::new(&ctx.t_args(
          "decisions.planLimit.startNote",
          [("connection", self.stop.connection.name().as_str())],
        ))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
      )
      .into()
  }

  /// *Start now on …*: the step starts on `chosen`'s account, a person's decision.
  fn start_button(&self, ctx: &Ctx, chosen: &AgentConnection, pending: bool) -> ActionButton {
    let name = self
      .accounts
      .of_connection(chosen.account(), chosen.provider())
      .map_or_else(
        || chosen.name().as_str().to_owned(),
        |account| account_name(ctx, account),
      );
    let button = ActionButton::new(
      "plan-limit-start-now",
      ButtonKind::Secondary,
      ctx.t_args("decisions.planLimit.startNow", [("account", name.as_str())]),
    )
    .icon(Icon::Play);
    if pending {
      return button.loading();
    }
    if !self.online {
      return button.disabled();
    }
    let (run, connection, gate) = (self.run.id(), chosen.id(), self.gate.cloned());
    let (asked, action) = (self.asked.clone(), self.action.clone());
    button.on_click(move || {
      if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        asked.set(Some(Continuing));
        action.run((Continuing, continue_on(run, connection)));
      } else {
        tracing::warn!("synthetic input started a waiting run on another account; not taken");
      }
    })
  }
}
