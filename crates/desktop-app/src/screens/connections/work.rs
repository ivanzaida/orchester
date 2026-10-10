//! Work the inspector starts on a stored connection, off the UI thread and listed in the status bar's
//! activity: checking its program again (Test connection, Refresh models, Check again), making the project
//! expect the account `gh` uses now (*Use this account*, `P89Dl`), turning an MCP server on or off, and
//! removing one no profile chooses. Turning a server on widens what agents get: while an MCP client's
//! window input is being applied it is taken only if a person lets automation change agent permissions, as
//! `set_mcp_server_enabled` would be.

use std::path::{Path, PathBuf};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_core::{id::ConnectionId, store::McpServerStore};
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::OperationSpec,
  background,
  connections::{
    data::recheck_in,
    github::expect_active_account_in,
    mcp::{enabling, remove_in, set_enabled_in},
  },
  messages::{self, Message},
  workspace::Workspace,
};

use crate::services::Services;

/// What runs on a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Work {
  Check,
  UseActiveAccount,
  /// Turn the MCP server on (`true`) or off for every profile that chooses it.
  SetEnabled(bool),
  Remove,
}

/// One piece of work as started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Start {
  pub repo_root: PathBuf,
  pub id: ConnectionId,
  pub work: Work,
  pub spec: OperationSpec,
  /// An MCP client's window input brought the press, not a person.
  pub by_automation: bool,
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
  let services = Services::of(ctx);
  let (activity, workspace) = (services.activity, services.workspace);
  ctx.future_action(move |start: Start| {
    let (activity, workspace) = (activity.clone(), workspace.clone());
    async move {
      let Start {
        repo_root,
        id,
        work,
        spec,
        by_automation,
      } = start;
      let run = async move { background::run(move || run(&workspace, &repo_root, (id, work), by_automation)).await? };
      activity.track(spec, run).await.map(|()| Done { id, work })
    }
  })
}

fn run(
  workspace: &Workspace,
  repo_root: &Path,
  (id, work): (ConnectionId, Work),
  by_automation: bool,
) -> Result<(), Message> {
  match work {
    Work::Check => recheck_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot store the connection check");
      error.message()
    }),
    Work::UseActiveAccount => expect_active_account_in(repo_root, id).map_err(|error| {
      tracing::error!(%error, "cannot record the account gh uses as the expected one");
      error.message()
    }),
    Work::SetEnabled(enabled) => {
      if by_automation {
        person_decides(workspace, repo_root, id, enabled)?;
      }
      set_enabled_in(repo_root, id, enabled).map_err(|error| {
        tracing::error!(%error, "cannot turn the MCP server on or off");
        messages::store_error(&error)
      })
    }
    Work::Remove => remove_in(repo_root, id).map_err(|error| error.message()),
  }
}

/// Refuses turning server `id` on for an MCP client's input unless a person lets automation widen.
fn person_decides(workspace: &Workspace, repo_root: &Path, id: ConnectionId, enabled: bool) -> Result<(), Message> {
  let db = ProjectDb::open(repo_root).map_err(|error| messages::store_error(&error))?;
  let Some(server) = db.mcp_server(id).map_err(|error| messages::store_error(&error))? else {
    return Ok(());
  };
  let widens = !enabling(&server, enabled).is_empty();
  let allowed = workspace
    .automation_changes_permissions()
    .map_err(|error| messages::store_error(&error))?;
  if widens && !allowed {
    tracing::warn!(server = %server.name(), "an MCP client's window input would turn an MCP server on; not taken");
    return Err(Message::new("appMcp.errors.personOnly"));
  }
  Ok(())
}
