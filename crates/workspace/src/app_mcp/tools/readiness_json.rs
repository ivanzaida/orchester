//! What the tools say of a resource's readiness on this computer (`docs/design/team-configuration.md`, "Readiness"),
//! as data a client matches on: `ready_here`, the hub's `group` and the ordered `missing` parts, each with the fix a
//! tool offers. One mapping of [`ProjectReadiness`] to JSON, so every tool that answers readiness names a part the
//! same way. It names ids, names of resources and CLI kinds only: no secret value, no keychain content, and no
//! program path, which no tool shows of a team MCP server (the team holds a program's name; each computer's path is
//! a person's fill-in).
//!
//! Every part has a `kind`, and `fix`:
//!
//! - `tool`: the tool that changes it, or `null` when no tool does;
//! - `gate`: who may make the change through it, `"person"` for what only a person does in the app's window (a tool
//!   that answers `person_only` always, or none) and `"ap"` when it needs *Let automation change agent permissions*;
//!   `null` when automation may.

use orchester_core::{
  id::ResourceId,
  resource::TeamProfileError,
  sharing::{ConnectionProblem, LocalProgramProblem, Missing, ProgramProblem},
};
use serde_json::{Value, json};

use crate::{
  app_mcp::{error::ToolError, services::Services},
  readiness::{Group, ProjectReadiness, ReadinessError, ReadinessStanding, ResourceReadiness, Undecided},
};

/// The readiness of every resource of the project at `repo_root`, read once for a tool call. Blocks: it reads the
/// databases and the filesystem, and the keychain for the secret of each tool of a local project that uses one.
pub(in crate::app_mcp) fn read(
  services: &Services,
  repo_root: &std::path::Path,
) -> Result<ProjectReadiness, ToolError> {
  services
    .workspace
    .project_readiness(repo_root, services.vault.get())
    .map_err(|ReadinessError::Store(error)| ToolError::from(error))
}

/// Resource `id` as a tool answers it; `null` for one with no version this computer uses, which `all` does not list.
pub(in crate::app_mcp) fn resource_of(all: &ProjectReadiness, id: ResourceId) -> Value {
  all.resource(id).map_or(Value::Null, |found| resource_json(found, all))
}

/// `found` as a tool answers it. `all` resolves the names of the MCP servers its parts refer to.
fn resource_json(found: &ResourceReadiness, all: &ProjectReadiness) -> Value {
  let missing = match found.standing() {
    ReadinessStanding::Ready => Vec::new(),
    ReadinessStanding::Missing(readiness) => readiness.missing().iter().map(|part| missing_json(part, all)).collect(),
    ReadinessStanding::Undecided(why) => vec![undecided_json(why)],
  };
  json!({
    "ready_here": found.standing().is_ready(),
    "group": group_name(found.group()),
    "version": found.number().get(),
    "missing": missing,
  })
}

const fn group_name(group: Group) -> &'static str {
  match group {
    Group::NeedsYouHere => "needs_you_here",
    Group::Ready => "ready",
    Group::OffHere => "off_here",
  }
}

fn missing_json(part: &Missing, all: &ProjectReadiness) -> Value {
  match part {
    Missing::Connection { cli, problem, .. } => match problem {
      ConnectionProblem::NoneOfKind => part_of(
        "connection",
        json!({ "cli": cli, "problem": "none_of_kind" }),
        fix(Some("add_cli_connection"), None),
      ),
      ConnectionProblem::PickNeeded => part_of(
        "connection",
        json!({ "cli": cli, "problem": "pick_needed" }),
        fix(Some("pick_profile_connection"), None),
      ),
    },
    Missing::McpOff { server } => part_of(
      "mcp_off",
      json!({ "server": server_json(*server, all) }),
      fix(Some("set_mcp_server_enabled"), Some("person")),
    ),
    Missing::McpProgram { server, problem } => {
      let problem = match problem {
        ProgramProblem::NotSet => "not_set",
        ProgramProblem::FileGone(_) => "file_gone",
      };
      part_of(
        "mcp_program",
        json!({ "server": server_json(*server, all), "problem": problem }),
        fix(None, Some("person")),
      )
    }
    Missing::Secret { name } => part_of("secret", json!({ "name": name.as_str() }), fix(None, Some("person"))),
    Missing::LocalMcpProgram { server, name, problem } => {
      let problem = match problem {
        LocalProgramProblem::NotFound => "not_found",
        LocalProgramProblem::Moved => "moved",
      };
      part_of(
        "local_mcp_program",
        json!({ "server": { "id": server.to_string(), "name": name.as_str() }, "problem": problem }),
        fix(Some("update_mcp_server"), Some("ap")),
      )
    }
    Missing::LocalSecret { secret, name } => part_of(
      "local_secret",
      json!({ "secret": { "id": secret.to_string(), "name": name.as_str() } }),
      fix(None, Some("person")),
    ),
  }
}

/// The team MCP server `id` by its id and, when the project lists it, its name.
fn server_json(id: ResourceId, all: &ProjectReadiness) -> Value {
  json!({ "id": id.to_string(), "name": all.resource(id).map(ResourceReadiness::name) })
}

fn part_of(kind: &str, mut fields: Value, fix: Value) -> Value {
  fields["kind"] = json!(kind);
  fields["fix"] = fix;
  fields
}

fn fix(tool: Option<&str>, gate: Option<&str>) -> Value {
  json!({ "tool": tool, "gate": gate })
}

/// A resource whose readiness cannot be known: not ready, and nothing here fixes it by itself.
fn undecided_json(why: &Undecided) -> Value {
  let mut part = json!({ "kind": "undecided", "fix": null });
  match why {
    Undecided::NotTeamForm => part["why"] = json!("not_team_form"),
    Undecided::CannotBePublished(error) => {
      part["why"] = json!("cannot_be_published");
      part["reason"] = json!(publish_reason(error));
    }
    Undecided::ChoosesMcpServer(server) => {
      part["why"] = json!("chooses_mcp_server");
      part["server"] = json!(server.to_string());
    }
    Undecided::RunsUnknown(id) => {
      part["why"] = json!("runs_unknown");
      part["resource"] = json!(id.to_string());
    }
    Undecided::ConnectionGone(connection) => {
      part["why"] = json!("connection_gone");
      part["connection"] = json!(connection.to_string());
    }
    Undecided::TeamToolSecretById(secret) => {
      part["why"] = json!("team_tool_secret_by_id");
      part["secret"] = json!(secret.to_string());
    }
    Undecided::SecretGone(secret) => {
      part["why"] = json!("secret_gone");
      part["secret"] = json!(secret.to_string());
    }
    Undecided::SecretUnreadable(secret) => {
      part["why"] = json!("secret_unreadable");
      part["secret"] = json!(secret.to_string());
    }
    Undecided::Member { id, why } => {
      part["why"] = json!("member");
      part["member"] = json!(id.to_string());
      part["cause"] = undecided_json(why);
    }
  }
  part
}

fn publish_reason(error: &TeamProfileError) -> &'static str {
  match error {
    TeamProfileError::Duplicate { .. } => "duplicate",
    TeamProfileError::TooMany { .. } => "too_many",
    TeamProfileError::Connection(_) => "connection",
    TeamProfileError::LocalMcpServer(_) => "local_mcp_server",
    TeamProfileError::WritableFolders { .. } => "writable_folders",
  }
}
