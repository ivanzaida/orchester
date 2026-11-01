//! `project-settings`: what Project settings' drawn states need beyond `connections` (`Project settings · redesign`
//! `XGtkg` in the new `orchester.pen`). Kontur is linked to `acme/kontur` and expects the GitHub account `dana-k`,
//! while `gh` is signed in as `ivanzaida`, so General and the GitHub connection offer *Use ivanzaida* (`01b`, `14`).
//! Secrets in Development, Staging and Production (`06`): `DEEPSEEK_API_KEY` used by the tool `deploy-preview`,
//! `FIGMA_TOKEN` rotated and used by the MCP server `figma`, `GITHUB_APP_KEY` used by nothing, and `OLD_FIGMA_TOKEN`
//! archived (`06b` opens `FIGMA_TOKEN`). The server `figma` (`13`) is a program that is on, with `FIGMA_API_KEY` from
//! `FIGMA_TOKEN`, tools that run without asking, and the profile Design reviewer using it. Every secret is a record
//! only: no value is in any keychain.
//!
//! `project-settings-shared` is the same project linked to a team server project at `orchester.example.com/kontur`
//! (`04b`), as owner, with four members as last read; no server answers, and this computer's list of servers doesn't
//! have it, so the page names it by its address and says what waits for it.

use std::collections::{BTreeMap, HashMap};

use orchester_core::{
  connection::{
    GitHubHost, GitHubLogin,
    mcp::{
      ExemptTools, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer,
    },
  },
  id::{ConnectionId, ProjectId, SecretId, ServerId, UserId},
  project::ProjectKey,
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, CommandArg, CommandProgram, DeniedBehavior, HttpAuth,
    HttpMethod, HttpRequest, ModelId, PromptText, ProviderSettings, Slug, Tool, ToolAction, VariableName,
  },
  run::BranchName,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  sharing::MemberRole,
  store::{McpServerStore, RepositoryStore, ResourceStore, SecretStore},
};
use orchester_store::{SharedMember, SharingLink};

use super::connections;
use crate::kontur::{Kontur, SeedError, catalogue, name};

/// The account the project expects through GitHub; `gh` is signed in as another (`connections::github`).
const EXPECTED: &str = "dana-k";
const DAY: i64 = 24 * 60;

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let connections = connections::seeded(kontur)?;
  link_repository(kontur, connections.github)?;
  let secrets = secrets(kontur)?;
  deploy_preview(kontur, secrets.deepseek)?;
  let figma = figma(kontur, secrets.figma)?;
  design_reviewer(kontur, (connections.codex, figma))
}

/// `project-settings`, linked to a team server project that no server here answers for.
pub fn shared(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let me = UserId::generate();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").map_err(invalid)?,
    role: Some(MemberRole::Admin),
    joined_at: kontur.ago(30 * DAY),
    cursor: 0,
    last_synced_at: kontur.ago(20),
    removed_at: None,
    address: Some("https://orchester.example.com/kontur".to_owned()),
  };
  kontur.db.start_sharing(&link, &[], (&[], &HashMap::new()))?;
  let added = kontur.ago(30 * DAY);
  let member = |user, (display, username, role): (&str, &str, MemberRole)| SharedMember {
    user,
    display_name: display.to_owned(),
    username: username.to_owned(),
    role,
    added_at: added,
  };
  let members = [
    member(me, ("Dana Kovalenko", EXPECTED, MemberRole::Admin)),
    member(UserId::generate(), ("Ivan", "ivan", MemberRole::Member)),
    member(UserId::generate(), ("Olha", "olha", MemberRole::Member)),
    member(UserId::generate(), ("Petro", "petro", MemberRole::Member)),
  ];
  kontur.db.record_shared_members(&members, kontur.ago(20), Some(me))?;
  Ok(())
}

/// The project's repository, `acme/kontur` through `github`, expecting [`EXPECTED`].
fn link_repository(kontur: &mut Kontur, github: ConnectionId) -> Result<(), SeedError> {
  let repo = GitHubRepo::new(GitHubHost::github_com(), "acme", "kontur").map_err(invalid)?;
  let view = RepositoryView {
    repo,
    default_branch: BranchName::new("main").map_err(invalid)?,
    permission: RepoPermission::Write,
  };
  let expected = GitHubLogin::new(EXPECTED).map_err(invalid)?;
  let link = RepositoryLink::new(github, RemoteName::origin(), view, expected, kontur.ago(60));
  kontur.db.save_repository_link(&link)?;
  Ok(())
}

struct Secrets {
  deepseek: SecretId,
  figma: SecretId,
}

/// The secrets of `06`, in three environments; records only.
fn secrets(kontur: &mut Kontur) -> Result<Secrets, SeedError> {
  let rows = [
    (
      "DEEPSEEK_API_KEY",
      SecretKind::ApiKey,
      Environment::Development,
      (3 * DAY, None, None),
    ),
    (
      "FIGMA_TOKEN",
      SecretKind::AccessToken,
      Environment::Development,
      (12 * DAY, Some(5 * DAY), None),
    ),
    (
      "GITHUB_APP_KEY",
      SecretKind::ApiKey,
      Environment::Development,
      (60, None, None),
    ),
    (
      "OLD_FIGMA_TOKEN",
      SecretKind::AccessToken,
      Environment::Development,
      (40 * DAY, None, Some(5 * DAY)),
    ),
    (
      "STAGING_DB_URL",
      SecretKind::DatabaseConnection,
      Environment::Staging,
      (20 * DAY, Some(2 * DAY), None),
    ),
    (
      "RELEASE_SIGNING_PASSWORD",
      SecretKind::Password,
      Environment::Production,
      (30 * DAY, None, None),
    ),
  ];
  let mut ids = Vec::new();
  for (named, kind, environment, (created, rotated, archived)) in rows {
    let new = NewSecret {
      name: SecretName::new(named).map_err(invalid)?,
      kind,
      environment,
    };
    let secret = Secret::from_stored(
      SecretId::generate(),
      new,
      kontur.ago(created),
      rotated.map(|minutes| kontur.ago(minutes)),
      archived.map(|minutes| kontur.ago(minutes)),
    );
    kontur.db.create_secret(&secret)?;
    ids.push(secret.id());
  }
  Ok(Secrets {
    deepseek: ids[0],
    figma: ids[1],
  })
}

/// The tool `deploy-preview`, an HTTP request with the bearer key in `key`.
fn deploy_preview(kontur: &mut Kontur, key: SecretId) -> Result<(), SeedError> {
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://preview.example.test/deploy".into(),
    "preview.example.test".into(),
    HttpAuth::Bearer { secret: key },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .map_err(invalid)?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    name("Development"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .map_err(invalid)?;
  let tool = Tool::new("", ToolAction::Http(request), Some(policy)).map_err(invalid)?;
  let at = kontur.ago(3 * DAY);
  kontur.db.create_resource(
    Slug::new("deploy-preview").map_err(invalid)?,
    name("deploy-preview"),
    &tool,
    at,
  )?;
  Ok(())
}

/// `figma` (`13`): `npx figma-developer-mcp --stdio`, on, with `FIGMA_API_KEY` from `token` and two tools that run
/// without asking; its id.
fn figma(kontur: &mut Kontur, token: SecretId) -> Result<ConnectionId, SeedError> {
  let args = ["figma-developer-mcp", "--stdio"]
    .iter()
    .map(|arg| CommandArg::new(arg).map_err(invalid))
    .collect::<Result<Vec<_>, _>>()?;
  let program = CommandProgram::new("npx").map_err(invalid)?;
  let key = VariableName::new("FIGMA_API_KEY").map_err(invalid)?;
  let environment = McpEnvironment::new(BTreeMap::from([(key, McpEnvValue::Secret(token))])).map_err(invalid)?;
  let mut server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("figma").map_err(invalid)?,
    McpTransport::Stdio(StdioServer::new(program, args, None)),
    environment,
    kontur.ago(12 * DAY),
  )
  .map_err(invalid)?;
  server.set_exempt_tools(ExemptTools::parse("get_figma_data\nget_screenshot").map_err(invalid)?);
  kontur.db.create_mcp_server(&server)?;
  Ok(server.id())
}

/// Design reviewer, a profile draft on Codex whose agents get `figma`.
fn design_reviewer(kontur: &mut Kontur, (codex, figma): (ConnectionId, ConnectionId)) -> Result<(), SeedError> {
  let servers = McpServerSelection::new(vec![figma]).map_err(invalid)?;
  let body = AgentProfile::new(
    name("Design reviewer"),
    String::new(),
    PromptText::new("Review the screens against the design.").map_err(invalid)?,
    codex,
    ModelId::new(catalogue::MODEL).map_err(invalid)?,
    ProviderSettings::default(),
  )
  .with_mcp_servers(servers);
  let at = kontur.ago(4);
  kontur.db.create_resource(
    Slug::new("design-reviewer").map_err(invalid)?,
    name("Design reviewer"),
    &body,
    at,
  )?;
  Ok(())
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
