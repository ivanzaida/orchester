//! Reading the usage and building the dashboard off the UI thread, for a project's page and for every
//! project's (`Usage` in *All projects*): the usage is read again whenever what it depends on changes, the
//! dashboard again whenever the query or the usage read does. Both are told apart by identity, so the time a
//! dashboard was built at never makes a section draw again.

use std::{path::PathBuf, sync::Arc};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureState},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  background,
  messages::{self, Message},
  usage::{Dashboard, Scope, UsageData, UsageQuery},
};

use crate::services::Services;

/// The usage read.
#[derive(Clone)]
pub struct Read(Arc<UsageData>);

impl PartialEq for Read {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Read {}

/// A dashboard built off the UI thread.
#[derive(Clone)]
pub struct Built(pub Arc<Dashboard>);

impl PartialEq for Built {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Built {}

/// Why the usage could not be read or charted: in words, and as the store reported it when it did
/// (`orchester.db: database is locked`, `Load failed` in `cK32U`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFailure {
  pub message: Message,
  pub reported: Option<Arc<str>>,
}

impl DevtoolsInspectable for ReadFailure {}

impl From<Message> for ReadFailure {
  fn from(message: Message) -> Self {
    Self {
      message,
      reported: None,
    }
  }
}

/// The usage from `since` on of the project at `root`, or of every project without one, read off the UI
/// thread again whenever `deps` change.
pub fn read<D>(ctx: &mut Ctx, root: Option<PathBuf>, since: Timestamp, deps: D) -> FutureState<Read, ReadFailure>
where
  D: Clone + PartialEq + Send + Sync + 'static,
{
  let services = Services::of(ctx);
  let (workspace, hold) = (services.workspace.clone(), services.hold);
  ctx
    .future((root, since, deps), move |(root, since, _)| {
      let (workspace, hold) = (workspace.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || {
          let scope = root.as_deref().map_or(Scope::All, Scope::Project);
          UsageData::read(&workspace, scope, since, Timestamp::now())
        })
        .await?
        .map(|data| Read(Arc::new(data)))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the usage");
          ReadFailure {
            message: messages::store_error(&error),
            reported: Some(error.to_string().into()),
          }
        })
      }
    })
    .state()
    .get()
}

/// The dashboard of the project named `name`, or of every project, for `query` over the usage `read`; none
/// before the usage is read.
pub fn build(
  ctx: &mut Ctx,
  (name, query): (Option<Arc<str>>, UsageQuery),
  read: Option<Read>,
) -> FutureState<Option<Built>, ReadFailure> {
  ctx
    .future((name, query, read), |(name, query, read)| async move {
      let Some(Read(data)) = read else {
        return Ok(None);
      };
      let built = move || Some(Built(Arc::new(Dashboard::of(name, &query, &data, Timestamp::now()))));
      Ok(background::run(built).await?)
    })
    .state()
    .get()
}

/// Why nothing can be charted: the read failed with nothing read before, or the dashboard could not be
/// built.
pub fn failure(
  read: &FutureState<Read, ReadFailure>,
  built: &FutureState<Option<Built>, ReadFailure>,
  shown: bool,
) -> Option<ReadFailure> {
  read
    .error
    .clone()
    .filter(|_| read.is_rejected() && !shown)
    .or_else(|| built.error.clone().filter(|_| built.is_rejected()))
}
