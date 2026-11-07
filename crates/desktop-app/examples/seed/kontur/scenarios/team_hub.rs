//! `team-hub`: the project of `shared-config` (the person is dana-k, the project's owner, on `Acme team`) with the
//! team's configuration in the model of `docs/design/team-configuration.md`, in every state the Agents hub groups by
//! (`Agents · v2` `acFkz`, A1–A6, ORC-831). MCP servers: `pencil-design` is on here without a program path, `github`
//! on here with its secret header `GITHUB_TOKEN` without a value, `figma` on here and ready, `playwright` switched off
//! here and `linear` nobody decided on. Profiles: Builder chooses `github` and `pencil-design`, Reviewer nothing.
//! Tools: `cargo-check`, a command, is ready; `sentry-issues`, an HTTP request with a project secret by id, cannot be
//! told ready. Instructions and skills are always ready. The old catalogue's own resources are local, so the team
//! project does not show them.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, SecretId, UserId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, ApprovalMode, CommandAction, CommandProgram,
    DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope, ModelId,
    PromptText, ProviderSettings, ResourceBody, Skill, Slug, TeamAgentProfile, TeamMcpServer, Tool, ToolAction,
    VersionNumber,
  },
  secret::SecretName,
  sharing::{SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::CachedVersion;
use serde_json::{Value, json};

use super::shared_config;
use crate::kontur::{Kontur, SeedError};

/// A hash the cached versions carry; the seed never compares them.
const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

/// Whether the person switched a team MCP server on or off here, or nobody decided.
#[derive(Clone, Copy)]
enum Here {
  On,
  Off,
  Undecided,
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared_config::admin(kontur)?;
  let program =
    json!({"name": "pencil-design", "launch": {"kind": "program", "program": "pencil-mcp", "variables": []}});
  let github = remote("github", Some("GITHUB_TOKEN"));
  let servers = [
    ("pencil-design", program, Here::On),
    ("github", github, Here::On),
    ("figma", remote("figma", None), Here::On),
    ("playwright", remote("playwright", None), Here::Off),
    ("linear", remote("linear", None), Here::Undecided),
  ];
  let mut ids = BTreeMap::new();
  for (name, body, here) in servers {
    let id = team_server(kontur, name, body)?;
    match here {
      Here::On => kontur.db.set_team_mcp_on_here(id, true)?,
      Here::Off => kontur.db.set_team_mcp_on_here(id, false)?,
      Here::Undecided => {}
    }
    ids.insert(name, id);
  }
  let builder = [ids["github"], ids["pencil-design"]];
  team_profile(kontur, "Builder", &builder, &["GITHUB_TOKEN"])?;
  team_profile(kontur, "Reviewer", &[], &[])?;
  plain_resources(kontur)
}

/// Instructions and skills, which need nothing of a computer, and the two tools.
fn plain_resources(kontur: &mut Kontur) -> Result<(), SeedError> {
  let rules = Instruction::new(
    PromptText::new("Work inside the task's worktree and keep each commit small.").map_err(invalid)?,
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .map_err(invalid)?;
  team_resource(kontur, "Project rules", &rules)?;
  let skill = Skill::new(
    "Review a diff against the brief and the acceptance criteria.".into(),
    "# Review a diff\nReport each finding with its file and line.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .map_err(invalid)?;
  team_resource(kontur, "review-diff", &skill)?;
  let command = CommandAction::new(CommandProgram::new("cargo").map_err(invalid)?, Vec::new());
  team_resource(kontur, "cargo-check", &Tool::command("", command))?;
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://sentry.example.test/api/issues".into(),
    "sentry.example.test".into(),
    HttpAuth::Bearer {
      secret: SecretId::generate(),
    },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .map_err(invalid)?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Production").map_err(invalid)?,
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .map_err(invalid)?;
  let tool = Tool::new("", ToolAction::Http(request), Some(policy)).map_err(invalid)?;
  team_resource(kontur, "sentry-issues", &tool)?;
  Ok(())
}

/// The team's profile `name` on Claude Code, choosing `servers` and needing the team secrets `secrets`; this computer
/// runs it by a local body, whose connection is not chosen.
fn team_profile(kontur: &mut Kontur, name: &str, servers: &[ResourceId], secrets: &[&str]) -> Result<(), SeedError> {
  let secrets = secrets
    .iter()
    .map(|secret| SecretName::new(secret))
    .collect::<Result<Vec<_>, _>>()
    .map_err(invalid)?;
  let team = TeamAgentProfile::new(
    Name::new(name).map_err(invalid)?,
    String::new(),
    PromptText::new("Do the work the pipeline step asks.").map_err(invalid)?,
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-sonnet-5-5").map_err(invalid)?,
      ProviderSettings::default(),
    ),
  )
  .with_mcp_servers(servers.to_vec())
  .map_err(invalid)?
  .with_secrets(secrets)
  .map_err(invalid)?;
  let resource = kontur.db.create_resource(
    Slug::new(&format!("team-{}", name.to_lowercase())).map_err(invalid)?,
    Name::new(name).map_err(invalid)?,
    &AgentProfile::from_team(&team, ConnectionId::generate()),
    kontur.ago(20 * 24 * 60),
  )?;
  let local = AgentProfile::from_team(&team, AgentProfile::unresolved_connection(resource.id()));
  let server_body = serde_json::to_value(&team).map_err(invalid)?;
  cache(kontur, &resource, server_body, local)
}

/// The team's resource `name` of any kind whose body is the same on the server and here, at v1.
pub(super) fn team_resource<B: ResourceBody + Clone>(
  kontur: &mut Kontur,
  name: &str,
  body: &B,
) -> Result<(), SeedError> {
  let resource = kontur.db.create_resource(
    Slug::new(&name.to_lowercase().replace(' ', "-")).map_err(invalid)?,
    Name::new(name).map_err(invalid)?,
    body,
    kontur.ago(20 * 24 * 60),
  )?;
  let server_body = serde_json::to_value(body).map_err(invalid)?;
  cache(kontur, &resource, server_body, body.clone())
}

/// Marks `resource` the team's and caches its v1 as the server confirmed it, published by olha.
fn cache<B: ResourceBody>(
  kontur: &mut Kontur,
  resource: &orchester_core::resource::Resource,
  server_body: Value,
  local_body: B,
) -> Result<(), SeedError> {
  kontur
    .db
    .keep_shared_resource(resource, TaskRevision::try_from(1_u64).map_err(invalid)?)?;
  let cached = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: kontur.ago(20 * 24 * 60),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body,
    hash: ContentHash::new(HASH).map_err(invalid)?,
    references: Vec::new(),
    local_body,
    waits_here: false,
  };
  kontur.db.cache_shared_version(resource.id(), &cached)?;
  Ok(())
}

/// A remote team MCP server, with one secret header taking `secret` when it has one.
fn remote(name: &str, secret: Option<&str>) -> Value {
  let headers: Vec<Value> = secret
    .map(|secret| json!({"header": "Authorization", "secret": secret, "prefix": "Bearer "}))
    .into_iter()
    .collect();
  json!({"name": name, "launch": {
    "kind": "remote", "url": format!("https://mcp.example.org/{name}"), "secret_headers": headers
  }})
}

/// The team's MCP server `name` at v1, as the server confirmed it, published by olha.
fn team_server(kontur: &mut Kontur, name: &str, body: Value) -> Result<ResourceId, SeedError> {
  let server: TeamMcpServer = serde_json::from_value(body.clone()).map_err(invalid)?;
  let resource = kontur.db.create_resource(
    Slug::new(name).map_err(invalid)?,
    Name::new(name).map_err(invalid)?,
    &server,
    kontur.ago(20 * 24 * 60),
  )?;
  cache(kontur, &resource, body, server)?;
  Ok(resource.id())
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
