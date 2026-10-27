//! The run's *Account* among its facts (`EwAhH`): the account the run's latest agent invocation started on, as the
//! runner recorded it, by its name and who was signed in; in `warning`, saying its limit is reached, while the run is
//! stopped at it. A removed account still names it: the run ran on it.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  node::Element,
};
use orchester_core::{account::AgentAccount, id::AgentAccountId};
use orchester_workspace::{background, messages};

use super::{
  properties::{Look, value},
  run_facts::fact,
};
use crate::services::Services;

/// The account, and whether the run is stopped at its plan limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Props(pub AgentAccountId, pub bool);

impl DevtoolsInspectable for Props {}

/// The account as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Option<AgentAccount>);

impl DevtoolsInspectable for Read {}

pub(super) struct RunAccount;

impl Component for RunAccount {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props(id, at_limit) = *ctx.props::<Props>();
    let workspace = Services::of(ctx).workspace;
    let read = ctx
      .future(id, move |id| {
        let workspace = workspace.clone();
        async move {
          background::run(move || workspace.agent_account(id))
            .await?
            .map(Read)
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let named = match read.data.and_then(|read| read.0) {
      Some(account) => {
        let name = account.name().map_or_else(
          || ctx.t("summary.thisRun.defaultAccount").to_string(),
          |name| name.as_str().to_owned(),
        );
        match account.email() {
          Some(email) => ctx
            .t_args(
              "summary.thisRun.accountValue",
              [("account", name.as_str()), ("email", email)],
            )
            .to_string(),
          None => name,
        }
      }
      None => ctx.t("summary.thisRun.accountUnknown").to_string(),
    };
    let key = ctx.t("summary.thisRun.account");
    if !at_limit {
      return fact(
        "task-this-run-account",
        &key,
        value("task-this-run-account-value", &named, Look::Secondary),
      );
    }
    let named = ctx.t_args("summary.thisRun.accountAtLimit", [("account", named.as_str())]);
    let shown = value("task-this-run-account-value", &named, Look::Secondary).color(PaletteColor::Warning);
    fact("task-this-run-account", &key, shown)
  }
}
