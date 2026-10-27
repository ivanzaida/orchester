//! Stopped at an account's plan limit (`Task · Summary · stopped at an account's plan limit` `EwAhH`), as a decision
//! card at the top of the Summary (`CqVv3`): *Continue this run* ([`card`]): the account's windows as its CLI last
//! reported them and the choice — wait for the account's reset (the default: the runner then resumes the same session
//! on the same account by itself) or continue now on another account of the same CLI, as a new session of the stopped
//! step for this run only. A running agent never moves to another account by itself. Then which account each step of
//! the run ran on ([`accounts`]). Every request needs the project's runner.

mod accounts;
mod card;
mod waiting;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::account::AgentAccount;
use orchester_workspace::{
  background, messages,
  tasks::{detail::RunView, plan_limit::PlanStop},
};

use super::{TaskContext, action::decision_action, arranged};
use crate::{mcp::input_gate::InputGate, services::accounts::listed as accounts_listed};

/// The request to continue, for its button to show it under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Continuing;

impl DevtoolsInspectable for Continuing {}

/// What the person chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
  /// Wait for the account's reset, then continue the same session.
  Wait,
  /// Continue now on another account.
  Other,
}

impl DevtoolsInspectable for Choice {}

/// The stop as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Option<PlanStop>);

impl DevtoolsInspectable for Read {}

pub struct PlanLimitDecision {
  gate: Option<InputGate>,
  choice: Signal<Choice>,
  /// The other connection chosen, by id.
  other: Signal<String>,
  asked: Signal<Option<Continuing>>,
}

impl Component for PlanLimitDecision {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      gate: ctx.use_context::<InputGate>(),
      choice: ctx.signal(Choice::Wait),
      other: ctx.signal(String::new()),
      asked: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = decision_action::<Continuing>(ctx, props.project.open.clone());
    let listed = accounts_listed(ctx);
    let Some(run) = props.detail.run.as_ref() else {
      return Column::new().into();
    };
    let stop = read(ctx, &props, run);
    // The account the stopped agent started on, as the runner recorded it.
    let account: Option<AgentAccount> = stop
      .as_ref()
      .and_then(|stop| stop.account)
      .and_then(|id| listed.of(id).cloned());
    // A step that waits before its agent starts is said by its own card (`XPjSc`), not as a stop.
    if let Some(stop) = stop.as_ref().filter(|stop| !stop.started) {
      let waiting = waiting::WaitingCard {
        run,
        stop,
        account: account.as_ref(),
        accounts: &listed,
        other: &self.other,
        asked: &self.asked,
        action: &action,
        online: props.online,
        gate: self.gate.as_ref(),
      }
      .render(ctx);
      return arranged(vec![waiting, accounts::section(ctx, run, &listed)]);
    }
    if let (Some(stop), Some(account)) = (stop.as_ref(), account.as_ref()) {
      words::publish(ctx, &props, run, (account, stop.reading.as_ref()));
    }
    let mut parts: Vec<Element> = Vec::new();
    if let Some(stop) = stop.as_ref() {
      parts.push(
        card::ChoiceCard {
          run,
          stop,
          account: account.as_ref(),
          choice: &self.choice,
          other: &self.other,
          asked: &self.asked,
          action: &action,
          online: props.online,
          gate: self.gate.as_ref(),
        }
        .render(ctx),
      );
    }
    parts.push(accounts::section(ctx, run, &listed));
    arranged(parts)
  }
}

/// What run `run` stopped at, read again with every change of the run.
fn read(ctx: &mut Ctx, props: &TaskContext, run: &RunView) -> Option<PlanStop> {
  let repo_root = props.project.open.repo_root.clone();
  ctx
    .future((run.id(), run.run.last_sequence().get()), move |(id, _)| {
      let repo_root = repo_root.clone();
      async move {
        background::run(move || PlanStop::load_in(&repo_root, id))
          .await?
          .map(Read)
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get()
    .data
    .and_then(|read| read.0)
}
