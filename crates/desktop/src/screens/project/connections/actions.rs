//! Work the connections screen starts on a stored connection, each reported to the status bar
//! activity: checking it again, pointing it at another program (gh not found, `ve4G9`) and
//! making the project expect the account `gh` uses now (*Use this account*, `P89Dl`), and turning an MCP
//! server on or off.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::id::ConnectionId;

use super::Selected;
use crate::{
  activity::{Activity, OperationSpec},
  background,
  connections::{
    data::recheck_in,
    github::{choose_program_in, expect_active_account_in},
    mcp::set_enabled_in,
  },
  messages::{self, Message},
};

/// The connection the work was done on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Done(pub ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Done {}

/// What runs on a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Work {
  Recheck,
  /// Point the connection at this program, then check it.
  Program(PathBuf),
  UseActiveAccount,
  /// Turn the MCP server on (`true`) or off for every profile that chooses it.
  SetEnabled(bool),
}

impl lurq::app::component::DevtoolsInspectable for Work {}

pub(super) type WorkAction = FutureAction<(PathBuf, ConnectionId, Work, OperationSpec), Done, Message>;

/// Runs `work` on connection `id` of the project at `repo_root` off the UI thread, tracked as
/// `spec`.
pub(super) fn work_action(ctx: &mut Ctx) -> WorkAction {
  let activity = Activity::of(ctx);
  ctx.future_action(
    move |(repo_root, id, work, spec): (PathBuf, ConnectionId, Work, OperationSpec)| {
      let activity = activity.clone();
      async move {
        let run = async move {
          let done = background::run(move || run(&repo_root, id, work)).await?;
          done.map(|()| Done(id))
        };
        activity.track(spec, run).await
      }
    },
  )
}

fn run(repo_root: &std::path::Path, id: ConnectionId, work: Work) -> Result<(), Message> {
  match work {
    Work::Recheck => recheck_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot store the connection check");
      error.message()
    }),
    Work::Program(executable) => choose_program_in(repo_root, id, executable).map_err(|error| {
      tracing::error!(%error, "cannot point the connection at the chosen program");
      error.message()
    }),
    Work::UseActiveAccount => expect_active_account_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot record the account gh uses as the expected one");
      error.message()
    }),
    Work::SetEnabled(enabled) => set_enabled_in(repo_root, id, enabled).map_err(|error| {
      tracing::error!(%error, "cannot turn the MCP server on or off");
      messages::store_error(&error)
    }),
  }
}

/// A handler that starts `work` on `id`, remembering which connection it runs on.
pub(super) fn start(
  action: &WorkAction,
  running: &lurq::core::Signal<Selected>,
  repo_root: &std::path::Path,
  id: ConnectionId,
  work: Work,
  spec: OperationSpec,
) -> Arc<dyn Fn() + Send + Sync> {
  let (action, running, repo_root) = (action.clone(), running.clone(), repo_root.to_path_buf());
  Arc::new(move || {
    running.set(Selected(Some(id)));
    action.run((repo_root.clone(), id, work.clone(), spec.clone()));
  })
}
