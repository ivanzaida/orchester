//! *Test again* and *Fetch models* in the native connection dialog (04.H1 `Byo8K`, 04.H1B `h2OUr`,
//! `docs/architecture/native-agent.md` §7): the project's runner tests the model API with the key secret
//! the dialog names, which the desktop never reads, and lists its models; the result shows beside the
//! button and the listed models join the dialog's models. A key typed into a new secret is not in the
//! keychain until the connection is saved, so it is tested after saving.

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::connection::{AgentProvider, ApiEndpoint, CheckResult, NativeModel};

use super::{
  actions::test_failure,
  native_dialog::{KeyChoice, NativeDialog},
};
use crate::{messages::Message, runner::Runners};

/// What a test found, and the models the model API listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Tested {
  pub(super) result: CheckResult,
  pub(super) models: Vec<NativeModel>,
}

impl lurq::app::component::DevtoolsInspectable for Tested {}

pub(super) type TestAction = FutureAction<(AgentProvider, ApiEndpoint), Tested, Message>;

/// The dialog's test through the runner.
pub(super) fn test_action(ctx: &mut Ctx) -> TestAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |(provider, api): (AgentProvider, ApiEndpoint)| {
    let runners = runners.clone();
    async move {
      runners
        .check_connection(provider, api)
        .await
        .map(|(result, models)| Tested { result, models })
        .map_err(|error| test_failure(&error))
    }
  })
}

impl NativeDialog {
  /// The model API the dialog's fields name, to test; why it cannot be tested yet otherwise.
  pub(super) fn tested_api(&self) -> Result<ApiEndpoint, Message> {
    let base_url = self.base_url()?;
    let key = match self.key.get() {
      KeyChoice::Existing(key) => key,
      KeyChoice::New => return Err(Message::new("connections.native.testAfterSaving")),
      KeyChoice::None => return Err(Message::new("connections.native.save.noKey")),
    };
    Ok(ApiEndpoint::new(self.preset.get().0, base_url, key))
  }
}
