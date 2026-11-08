//! `team-hub`: the project of `shared-config` (the person is dana-k, the project's owner, on `Acme team`) with the
//! team's configuration in the model of `docs/design/team-configuration.md`, in every state the Agents hub groups by
//! (`Agents · v2` `acFkz`, A1–A6, ORC-831). MCP servers: `pencil-design` is on here without a program path, `github`
//! on here with its secret header `GITHUB_TOKEN` without a value, `figma` on here and ready, `playwright` switched off
//! here and `linear` nobody decided on. Profiles: Builder chooses `github` and `pencil-design`, Reviewer nothing.
//! Tools: `cargo-check`, a command, is ready; `sentry-issues`, an HTTP request with a project secret by id, cannot be
//! told ready (A6 `u9Hcc`); `orders-report`, a database query, names its secret by id the same way. Builder uses
//! `Project rules`, `review-diff` (which has a reference file) and `cargo-check`. Instructions and skills are always
//! ready. The old catalogue's own resources are local, so the team project does not show them.
//!
//! The computer has one Claude Code connection (`Claude · Kontur`), so Builder runs on it without asking
//! (`Agents · Profile` C1 `NDZXC`). `team-hub-pick` adds a second one, `Claude · Home`, so Builder asks which runs it
//! (C2 `WPvZx`); `team-hub-no-connection` removes the Claude Code connection, so Builder says what to add (C3 `QOBF2`).
//!
//! `team-hub-member` is the same team for ivan, a member who is not an admin (`Agents · … · member` M1–M6, ORC-839).
//! Reach the frames in the Agents hub: M1 `oPF2o` Profiles › Reviewer with `team-hub-member-pick` (two Claude Code
//! connections, so *Runs on* is a picker; Builder under `team-hub-member` has a missing part and its bar); M2 `dijUt`
//! MCP servers › `pencil-design`; M3 `H9nUK6` MCP servers › `playwright` (off here); M4 `D8WX9` Instructions ›
//! `Project rules`; M5 `ITxzr` Skills under `team-hub-member-no-skills`; M6 `yTVeE` Tools › `sentry-issues`. M1 as
//! drawn, with the connection already picked and no bar, is Profiles › Reviewer under `team-hub-member-picked`.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
};

use orchester_core::{
  blob::ContentHash,
  connection::{AgentProvider, SignIn},
  id::{ConnectionId, ResourceId, SecretId, UserId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, ApprovalMode, CommandAction, CommandArg, CommandProgram,
    DatabaseQuery, DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope,
    ModelId, PromptText, ProviderSettings, ResourceBody, ResourceKind, Skill, SkillFile, SkillFilePath, Slug,
    TeamAgentProfile, TeamMcpServer, Tool, ToolAction, VersionNumber,
  },
  secret::SecretName,
  sharing::{SharedPerson, TaskRevision},
  store::{BlobStore, ResourceStore},
};
use orchester_store::{CachedVersion, ConnectionPick};
use serde_json::{Value, json};

use super::{connections, shared_config};
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

/// `team-hub-pick`: [`seed`] with a second Claude Code connection, so Builder's connection is the person's to pick.
pub fn pick(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  second_connection(kontur).map(|_| ())
}

/// `team-hub-member`: the team of [`seed`] read by ivan, a member of the project who is not an admin, on the same
/// resources and the same computer (`Agents · … · member` M1–M6 `oPF2o`, `dijUt`, `H9nUK6`, `D8WX9`, `yTVeE`, ORC-839):
/// no New, no Edit, the team's part read as values under the line naming the admins, and the computer's own part
/// still his.
pub fn member(kontur: &mut Kontur) -> Result<(), SeedError> {
  member_with(kontur, Kinds::ALL)
}

fn member_with(kontur: &mut Kontur, kinds: Kinds) -> Result<(), SeedError> {
  shared_config::member_over(kontur, |kontur| {
    resources_with(kontur, &Mcp::default(), kinds).map(|_| ())
  })
}

/// `team-hub-member-pick`: [`member`] with a second Claude Code connection, so the connection that runs Builder is a
/// live picker for him (M1 `oPF2o`).
pub fn member_pick(kontur: &mut Kontur) -> Result<(), SeedError> {
  member(kontur)?;
  second_connection(kontur).map(|_| ())
}

/// `team-hub-member-picked`: [`member_pick`] where he has already picked `Claude · Home` for Reviewer, as the store
/// keeps a person's pick, so Reviewer's *Runs on* shows the connection in its field and nothing is missing (M1
/// `oPF2o` as drawn: no bar).
pub fn member_picked(kontur: &mut Kontur) -> Result<(), SeedError> {
  member(kontur)?;
  let home = second_connection(kontur)?;
  let reviewer = kontur
    .db
    .kept_resources(ResourceKind::AgentProfile)?
    .into_iter()
    .find(|profile| profile.slug().as_str() == "team-reviewer")
    .ok_or_else(|| SeedError::Invalid("the seed has the team profile Reviewer".to_owned()))?;
  kontur.db.set_connection_pick(
    reviewer.id(),
    ConnectionPick {
      connection: home,
      account: None,
    },
  )?;
  Ok(())
}

/// `team-hub-member-no-skills`: [`member`] where the team has published no skill, so the Skills filter holds the one
/// empty state that says an admin adds them (M5 `ITxzr`).
pub fn member_no_skills(kontur: &mut Kontur) -> Result<(), SeedError> {
  member_with(kontur, Kinds { skills: false })
}

/// A second Claude Code connection, `Claude · Home`.
fn second_connection(kontur: &mut Kontur) -> Result<ConnectionId, SeedError> {
  let program = kontur.repo.join(".orchester").join("qa-bin").join("claude-home");
  fs::write(&program, "").map_err(invalid)?;
  connections::cli(
    kontur,
    ("Claude · Home", AgentProvider::ClaudeCodeCli, program),
    (
      "2.1.88",
      SignIn::SignedIn,
      &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"],
    ),
    kontur.ago(4),
  )
}

/// `team-hub-no-connection`: [`seed`] on a computer with no Claude Code connection, so the team's profiles say what to
/// add and nothing stands in for it. The store has no way to remove a connection, so the seed's own row goes, with the
/// picks that name it; the seed has exactly one such connection, and anything else is an error.
pub fn no_connection(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let mut database = rusqlite::Connection::open(kontur.database())?;
  let removal = database.transaction()?;
  removal.execute(
    "DELETE FROM team_connection_picks WHERE connection_id IN (SELECT id FROM connections WHERE provider = ?1)",
    ["claude_code_cli"],
  )?;
  let removed = removal.execute("DELETE FROM connections WHERE provider = ?1", ["claude_code_cli"])?;
  if removed != 1 {
    return Err(SeedError::Invalid(format!(
      "the seed has one Claude Code connection to remove, found {removed}"
    )));
  }
  removal.commit()?;
  Ok(())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed_with(kontur, &Mcp::default()).map(|_| ())
}

/// How the program MCP servers of the scenario differ (`team-hub-mcp-*`, C4–C8 in `acFkz`): the variables of
/// `pencil-design`, and whether `figma` runs a program (`npx`) instead of being reached at an address.
#[derive(Default)]
pub(super) struct Mcp {
  pub pencil_variables: &'static [&'static str],
  pub figma_program: bool,
}

/// [`seed`], which also names the id of each MCP server it made.
pub(super) fn seed_with(kontur: &mut Kontur, mcp: &Mcp) -> Result<BTreeMap<&'static str, ResourceId>, SeedError> {
  shared_config::admin(kontur)?;
  resources_with(kontur, mcp, Kinds::ALL)
}

/// Which kinds the team has published, of those a scenario may leave out.
#[derive(Clone, Copy)]
struct Kinds {
  skills: bool,
}

impl Kinds {
  const ALL: Self = Self { skills: true };
}

/// The team's servers, profiles, tools, instructions and skills of [`seed`], without the link to a team server and its
/// members, which a QA copy that joins a stand-in server makes itself.
pub(super) fn resources(kontur: &mut Kontur) -> Result<(), SeedError> {
  resources_with(kontur, &Mcp::default(), Kinds::ALL).map(|_| ())
}

/// [`resources`] with the MCP servers' differences and the kinds published, naming the id of each MCP server it made.
fn resources_with(
  kontur: &mut Kontur,
  mcp: &Mcp,
  kinds: Kinds,
) -> Result<BTreeMap<&'static str, ResourceId>, SeedError> {
  let program = json!({"name": "pencil-design", "launch": {
    "kind": "program", "program": "pencil-mcp", "args": ["--stdio", "--app", "desktop"],
    "variables": mcp.pencil_variables
  }, "exempt_tools": ["get_screenshot", "batch_get", "get_variables"]});
  let github = remote("github", Some("GITHUB_TOKEN"));
  let figma = if mcp.figma_program {
    json!({"name": "figma", "launch": {"kind": "program", "program": "npx", "variables": []}})
  } else {
    remote("figma", None)
  };
  let servers = [
    ("pencil-design", program, Here::On),
    ("github", github, Here::On),
    ("figma", figma, Here::On),
    ("playwright", remote("playwright", None), Here::Off),
    ("linear", remote("linear", None), Here::Undecided),
  ];
  let mut ids: BTreeMap<&'static str, ResourceId> = BTreeMap::new();
  for (name, body, here) in servers {
    let id = team_server(kontur, name, body)?;
    match here {
      Here::On => kontur.db.set_team_mcp_on_here(id, true)?,
      Here::Off => kontur.db.set_team_mcp_on_here(id, false)?,
      Here::Undecided => {}
    }
    ids.insert(name, id);
  }
  let plain = plain_resources(kontur, kinds)?;
  let builder = Uses {
    servers: &[ids["github"], ids["pencil-design"]],
    instructions: &[plain.rules],
    skills: plain.review.as_slice(),
    tools: &[plain.cargo],
  };
  team_profile(kontur, "Builder", &builder, &["GITHUB_TOKEN"])?;
  team_profile(kontur, "Reviewer", &Uses::default(), &[])?;
  Ok(ids)
}

/// What a team profile of the seed uses.
#[derive(Default)]
struct Uses<'a> {
  servers: &'a [ResourceId],
  instructions: &'a [ResourceId],
  skills: &'a [ResourceId],
  tools: &'a [ResourceId],
}

/// The ids of the instruction, the skill and the command tool the profiles use.
struct Plain {
  rules: ResourceId,
  /// The skill, when the team has published skills.
  review: Option<ResourceId>,
  cargo: ResourceId,
}

/// Instructions and skills, which need nothing of a computer, and the tools.
fn plain_resources(kontur: &mut Kontur, kinds: Kinds) -> Result<Plain, SeedError> {
  let rules = Instruction::new(
    PromptText::new("Work inside the task's worktree and keep each commit small.").map_err(invalid)?,
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .map_err(invalid)?;
  let rules = team_resource(kontur, "Project rules", &rules)?;
  let review = kinds.skills.then(|| review_skill(kontur)).transpose()?;
  let command = CommandAction::new(
    CommandProgram::new("cargo").map_err(invalid)?,
    vec![CommandArg::new("check").map_err(invalid)?],
  );
  let cargo = team_resource(
    kontur,
    "cargo-check",
    &Tool::command("Runs cargo check on the worktree.", command),
  )?;
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
  let database = DatabaseQuery::new(
    SecretId::generate(),
    "select status, count(*) from orders group by status".into(),
  )
  .map_err(invalid)?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Production").map_err(invalid)?,
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .map_err(invalid)?;
  let report = Tool::new("Orders per status.", ToolAction::DatabaseQuery(database), Some(policy)).map_err(invalid)?;
  team_resource(kontur, "orders-report", &report)?;
  Ok(Plain { rules, review, cargo })
}

/// The skill `review-diff`, which has a reference file.
fn review_skill(kontur: &mut Kontur) -> Result<ResourceId, SeedError> {
  let checklist = b"- Every acceptance criterion has a test.\n- No unrelated change.\n";
  let file = SkillFile {
    path: SkillFilePath::new("references/checklist.md").map_err(invalid)?,
    content: kontur.db.put_blob("text/markdown", checklist)?,
    size: checklist.len() as u64,
  };
  let skill = Skill::new(
    "Review a diff against the brief and the acceptance criteria.".into(),
    "# Review a diff\nReport each finding with its file and line.".into(),
    vec![file],
    InstructionScope::Project,
  )
  .map_err(invalid)?;
  team_resource(kontur, "review-diff", &skill)
}

/// The team's profile `name` on Claude Code, choosing `servers` and needing the team secrets `secrets`; this computer
/// runs it by a local body, whose connection is not chosen.
fn team_profile(kontur: &mut Kontur, name: &str, uses: &Uses<'_>, secrets: &[&str]) -> Result<(), SeedError> {
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
  .with_mcp_servers(uses.servers.to_vec())
  .map_err(invalid)?
  .with_instruction_resources(uses.instructions.to_vec())
  .map_err(invalid)?
  .with_skills(uses.skills.to_vec())
  .map_err(invalid)?
  .with_tools(uses.tools.to_vec())
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
) -> Result<ResourceId, SeedError> {
  let resource = kontur.db.create_resource(
    Slug::new(&name.to_lowercase().replace(' ', "-")).map_err(invalid)?,
    Name::new(name).map_err(invalid)?,
    body,
    kontur.ago(20 * 24 * 60),
  )?;
  let server_body = serde_json::to_value(body).map_err(invalid)?;
  cache(kontur, &resource, server_body, body.clone())?;
  Ok(resource.id())
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
