//! *Test again* and *Fetch models* in the native connection dialog (04.H1 `Byo8K`, 04.H1B `h2OUr`,
//! `docs/architecture/native-agent.md` §7): the project's runner tests the model API with the key secret
//! the dialog names, which the desktop never reads, and lists its models; the result shows beside the
//! button and the listed models join the dialog's models. A key typed into a new secret is not in the
//! keychain until the connection is saved, so it is tested after saving.

use std::sync::Arc;

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  connection::{AgentProvider, ApiEndpoint, CheckResult, NativeModel},
  time::Timestamp,
};

use super::{
  actions::test_failure,
  native_dialog::{KeyChoice, NativeDialog},
};
use crate::{display, messages::Message, runner::Runners};

/// What a test found, and the models the model API listed; the host it reached and when it ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Tested {
  pub(super) result: CheckResult,
  pub(super) models: Vec<NativeModel>,
  pub(super) host: Arc<str>,
  pub(super) at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for Tested {}

/// The test the dialog's test row describes: the one just run, or else the connection's last one.
pub(super) struct LastTest {
  pub(super) result: CheckResult,
  pub(super) host: Arc<str>,
  pub(super) at: Timestamp,
  /// How many models the model API listed; `None` for a test recorded before the dialog opened.
  pub(super) listed: Option<usize>,
}

impl LastTest {
  /// What the test found (*api.anthropic.com answered · 4 models listed · 14:02*), or the balance of a
  /// key with a billing problem.
  pub(super) fn found(&self, ctx: &Ctx) -> Option<Arc<str>> {
    let time = display::clock(self.at);
    match &self.result {
      CheckResult::Api { .. } => Some(match self.listed {
        Some(count) => ctx.t_args(
          if count == 1 {
            "connections.native.testListed.one"
          } else {
            "connections.native.testListed.other"
          },
          [
            ("host", &*self.host),
            ("count", count.to_string().as_str()),
            ("time", time.as_str()),
          ],
        ),
        None => ctx.t_args(
          "connections.native.testAnswered",
          [("host", &*self.host), ("time", time.as_str())],
        ),
      }),
      result => result
        .balance()
        .map(|balance| ctx.t_args("connections.native.testBalance", [("balance", balance)])),
    }
  }
}

pub(super) type TestAction = FutureAction<(AgentProvider, ApiEndpoint), Tested, Message>;

/// The dialog's test through the runner.
pub(super) fn test_action(ctx: &mut Ctx) -> TestAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(provider, api): (AgentProvider, ApiEndpoint)| {
    let runners = runners.clone();
    let host: Arc<str> = api.base_url().host().into();
    async move {
      runners
        .check_connection(provider, api)
        .await
        .map(|(result, models)| Tested {
          result,
          models,
          host,
          at: Timestamp::now(),
        })
        .map_err(|error| test_failure(&error))
    }
  })
}

impl NativeDialog {
  /// The model API the dialog's fields name, to test; why it cannot be tested yet otherwise, also when
  /// Orchester does not run this provider's agents yet.
  pub(super) fn tested_api(&self) -> Result<ApiEndpoint, Message> {
    if !self.provider.runs_agents() {
      return Err(Message::new("connections.native.testUnavailable"));
    }
    let base_url = self.base_url()?;
    let key = match self.key.get() {
      KeyChoice::Existing(key) => key,
      KeyChoice::New => return Err(Message::new("connections.native.testAfterSaving")),
      KeyChoice::None => return Err(Message::new("connections.native.save.noKey")),
    };
    Ok(ApiEndpoint::new(self.preset.get().0, base_url, key))
  }
}
