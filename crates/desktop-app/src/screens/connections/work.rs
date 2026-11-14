//! Work the inspector starts on a stored connection, off the UI thread and listed in the status bar's
//! activity: checking its program again (Check again, Refresh models), testing a native connection's model API
//! through the runner (Test connection, Fetch models), making the project expect the account `gh` uses now (*Use this
//! account*, `P89Dl`), turning an MCP server on or off, and removing one no profile chooses. Turning a server on widens
//! what agents get, so the inspector asks a person first (`widen`).

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::id::{ConnectionId, ProjectId};
use orchester_workspace::{
  activity::OperationSpec,
  background,
  connections::{
    data::recheck_in,
    github::expect_active_account_in,
    mcp::{remove_in, set_enabled_in},
  },
  messages::{self, Message},
  tasks::outside::Outside,
  workspace::Workspace,
};

use crate::services::Services;

/// What runs on a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Work {
  /// Check a CLI's or gh's program again.
  Check,
  UseActiveAccount,
  /// Turn the MCP server on (`true`) or off for every profile that chooses it.
  SetEnabled(bool),
  Remove,
}

impl Work {
  /// Whether the work finds out how the connection stands: a check of its program.
  pub(super) const fn is_check(self) -> bool {
    matches!(self, Self::Check)
  }
}

/// One piece of work as started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Start {
  pub project: ProjectId,
  pub repo_root: PathBuf,
  pub id: ConnectionId,
  pub work: Work,
  pub spec: OperationSpec,
}

/// The work that finished, and on which connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Done {
  pub id: ConnectionId,
  pub work: Work,
}

impl DevtoolsInspectable for Done {}

pub(super) type WorkAction = FutureAction<Start, Done, Message>;

/// The connection work of the screen; created at the same place in every render.
pub(super) fn action(ctx: &mut Ctx) -> WorkAction {
  let Services {
    activity,
    workspace,
    changes,
    runners,
    ..
  } = Services::of(ctx);
  ctx.future_action(move |start: Start| {
    let (activity, workspace, changes, _runners) =
      (activity.clone(), workspace.clone(), changes.clone(), runners.clone());
    async move {
      let Start {
        project,
        repo_root,
        id,
        work,
        spec,
      } = start;
      let run = async move {
        let blocking = match work {
          Work::Check => Blocking::Check,
          Work::UseActiveAccount => Blocking::UseActiveAccount,
          Work::SetEnabled(enabled) => Blocking::SetEnabled(enabled),
          Work::Remove => Blocking::Remove,
        };
        background::run(move || run(&workspace, &repo_root, (id, blocking))).await?
      };
      activity.track(spec, run).await?;
      // Every screen that shows the project's connections reads them again, the status bar's accounts too.
      changes.record(project, Outside::Connections);
      Ok(Done { id, work })
    }
  })
}

/// The [`Work`] that blocks on the project database or a program, run off the UI thread; a native test goes
/// through the runner instead.
enum Blocking {
  Check,
  UseActiveAccount,
  SetEnabled(bool),
  Remove,
}

fn run(_workspace: &Workspace, repo_root: &Path, (id, work): (ConnectionId, Blocking)) -> Result<(), Message> {
  match work {
    Blocking::Check => recheck_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot store the connection check");
      error.message()
    }),
    Blocking::UseActiveAccount => expect_active_account_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot record the account gh uses as the expected one");
      error.message()
    }),
    Blocking::SetEnabled(enabled) => set_enabled_in(repo_root, id, enabled).map_err(|error| {
      tracing::error!(%error, "cannot turn the MCP server on or off");
      messages::store_error(&error)
    }),
    Blocking::Remove => remove_in(repo_root, id).map_err(|error| error.message()),
  }
}
