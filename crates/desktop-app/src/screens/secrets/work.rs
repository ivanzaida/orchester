//! Credential writes through the workspace, away from the UI thread.
use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{
  id::SecretId,
  secret::{NewSecret, SecretValue},
};
use orchester_workspace::{
  activity::OperationSpec,
  background,
  messages::{self, Message},
  secrets::{
    Vault,
    data::{self, ReplacementOutcome, ReplacementPermission},
  },
};

use crate::services::Services;

#[derive(Clone)]
pub(super) enum Write {
  Add(NewSecret),
  Replace(SecretId, ReplacementPermission),
  Archive(SecretId),
}
pub(super) struct Start {
  pub repo: PathBuf,
  pub vault: Vault,
  pub write: Write,
  pub value: Option<SecretValue>,
  pub spec: OperationSpec,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  Saved(SecretId),
  NeedsPerson,
}
impl DevtoolsInspectable for Outcome {}
pub(super) type Action = FutureAction<Start, Outcome, Message>;

pub(super) fn action(ctx: &mut Ctx) -> Action {
  let activity = Services::of(ctx).activity;
  ctx.future_action(move |start: Start| {
    let activity = activity.clone();
    async move {
      let running = activity.start(start.spec.clone());
      let result = background::run(move || {
        let value = || {
          start
            .value
            .as_ref()
            .expect("add and replace supply a validated credential")
        };
        let result = match start.write {
          Write::Add(new) => data::add_in(&start.repo, start.vault.get(), new, value()).map(Outcome::Saved),
          Write::Replace(id, permission) => {
            data::replace_allowed_in(&start.repo, start.vault.get(), id, value(), permission).map(|outcome| {
              match outcome {
                ReplacementOutcome::Replaced => Outcome::Saved(id),
                ReplacementOutcome::NeedsPerson => Outcome::NeedsPerson,
              }
            })
          }
          Write::Archive(id) => data::archive_in(&start.repo, start.vault.get(), id).map(|()| Outcome::Saved(id)),
        };
        result.map_err(|error| messages::secret_error(&error))
      })
      .await
      .and_then(|result| result);
      match &result {
        Ok(Outcome::Saved(_)) => running.finish(),
        // No write took place; dropping the operation removes it without a success event.
        Ok(Outcome::NeedsPerson) => drop(running),
        Err(error) => running.fail(error.clone()),
      }
      result
    }
  })
}
