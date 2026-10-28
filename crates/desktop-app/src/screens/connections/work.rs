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
use orchester_core::id::{AgentAccountId, ConnectionId, ProjectId};
use orchester_workspace::{
  activity::OperationSpec,
  background,
  connections::{
    data::{recheck_in, set_account_in},
    github::expect_active_account_in,
    mcp::{remove_in, set_enabled_in},
    native::test_in,
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
  /// Test a native connection's model API through the runner, which also fetches its models.
  Test,
  UseActiveAccount,
  /// Turn the MCP server on (`true`) or off for every profile that chooses it.
  SetEnabled(bool),
  Remove,
  /// Run the agent connection on this account from now on, checked there.
  SetAccount(AgentAccountId),
}

impl Work {
  /// Whether the work finds out how the connection stands: a check of its program, or a test of its model API.
  pub(super) const fn is_check(self) -> bool {
    matches!(self, Self::Check | Self::Test)
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
    let (activity, workspace, changes, runners) =
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
        match work {
          Work::Test => test_in(&runners, repo_root, id).await,
          _ => background::run(move || run(&workspace, &repo_root, (id, work))).await?,
        }
      };
      activity.track(spec, run).await?;
      // Every screen that shows the project's connections reads them again, the status bar's accounts too.
      changes.record(project, Outside::Connections);
      Ok(Done { id, work })
    }
  })
}

fn run(workspace: &Workspace, repo_root: &Path, (id, work): (ConnectionId, Work)) -> Result<(), Message> {
  match work {
    Work::Test => unreachable!("`action` has the runner test a native connection and never hands that work here"),
    Work::SetAccount(account) => {
      let account = workspace
        .agent_account(account)
        .map_err(|error| messages::store_error(&error))?
        .ok_or_else(|| messages::store_error(&orchester_store::StoreError::AgentAccountNotFound(account)))?;
      set_account_in(repo_root, id, &account).map_err(|error| {
        tracing::error!(%error, "cannot run the connection on another account");
        error.message()
      })
    }
    Work::Check => recheck_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot store the connection check");
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
    Work::Remove => remove_in(repo_root, id).map_err(|error| error.message()),
  }
}
