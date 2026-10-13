//! *This run › Account* (`EwAhH`): the account the run's latest agent invocation started on, as the runner recorded
//! it, by its name and who was signed in; in `warning`, saying its limit is reached, while the run is stopped at it.
//! A removed account still names it: the run ran on it.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::Text,
  node::Element,
};
use orchester_core::{account::AgentAccount, id::AgentAccountId};
use orchester_workspace::{background, messages};

use super::this_run::row;
use crate::{
  services::Services,
  theme::ExtraTypography,
  ui::detail_row::{DetailValue, detail_row_of},
};

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
    let value = match read.data.and_then(|read| read.0) {
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
    if !at_limit {
      return row(ctx, "summary.thisRun.account", &value, DetailValue::Text);
    }
    let value = ctx.t_args("summary.thisRun.accountAtLimit", [("account", value.as_str())]);
    detail_row_of(
      "task-this-run-account",
      &ctx.t("summary.thisRun.account"),
      Text::new(&value)
        .id("task-this-run-account-value")
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::Warning)
        .flex(1.0)
        .min_width(lurq::node::dimension::Dimension::Px(0.0)),
    )
    .into()
  }
}
