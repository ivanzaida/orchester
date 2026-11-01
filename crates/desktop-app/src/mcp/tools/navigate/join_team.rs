//! `navigate` to Join a team (`route` join_team): the way it opens on, and for a joined team's shared projects and a
//! folder to link to one of them, which server and project.

use serde_json::{Map, Value};

use super::super::{
  ToolError,
  settings_route::{self, Lookup},
  string,
};
use crate::routes::{JoinTarget, JoinWay, Route};

/// The parameters besides `route` that join_team takes.
pub(super) const PARAMS: &[&str] = &["way", "server", "project"];

/// Join a team on the way `way` names, Orchester Cloud when left out; `server` and `project` only where it takes them.
pub(super) fn route(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<Route, ToolError> {
  let way = string(args, "way")?;
  let takes: &[&str] = match way {
    Some("projects") => &["server"],
    Some("folder") => &["server", "project"],
    _ => &[],
  };
  if let Some(param) = ["server", "project"]
    .into_iter()
    .find(|param| args.contains_key(*param) && !takes.contains(param))
  {
    return Err(ToolError::UnexpectedParam {
      of: "join_team",
      param: param.to_owned(),
    });
  }
  let server = |lookup| {
    let named = string(args, "server")?.ok_or(ToolError::Missing { param: "server" })?;
    settings_route::server(named, lookup)
  };
  let way = match way {
    None | Some("cloud") => JoinWay::Cloud,
    Some("code") => JoinWay::Code,
    Some("projects") => JoinWay::Projects(server(lookup)?),
    Some("folder") => {
      let slug = string(args, "project")?.ok_or(ToolError::Missing { param: "project" })?;
      let target = JoinTarget::new(server(lookup)?, slug).ok_or(ToolError::Missing { param: "project" })?;
      JoinWay::Folder(target)
    }
    Some(other) => {
      return Err(ToolError::InvalidParam {
        param: "way",
        reason: format!("`{other}` is not cloud, code, projects or folder"),
      });
    }
  };
  Ok(Route::JoinTeam(way))
}
