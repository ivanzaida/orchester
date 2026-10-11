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
  secrets::{Vault, data},
};

use crate::services::Services;

#[derive(Clone)]
pub(super) enum Write {
  Add(NewSecret),
  Replace(SecretId),
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
pub(super) struct Saved(pub SecretId);
impl DevtoolsInspectable for Saved {}
pub(super) type Action = FutureAction<Start, Saved, Message>;

pub(super) fn action(ctx: &mut Ctx) -> Action {
  let activity = Services::of(ctx).activity;
  ctx.future_action(move |start: Start| {
    let activity = activity.clone();
    async move {
      let spec = start.spec.clone();
      let write = async move {
        background::run(move || {
          let value = || {
            start
              .value
              .as_ref()
              .expect("add and replace supply a validated credential")
          };
          let result = match start.write {
            Write::Add(new) => data::add_in(&start.repo, start.vault.get(), new, value()),
            Write::Replace(id) => data::replace_in(&start.repo, start.vault.get(), id, value()).map(|()| id),
            Write::Archive(id) => data::archive_in(&start.repo, start.vault.get(), id).map(|()| id),
          };
          result.map(Saved).map_err(|error| messages::secret_error(&error))
        })
        .await?
      };
      activity.track(spec, write).await
    }
  })
}
