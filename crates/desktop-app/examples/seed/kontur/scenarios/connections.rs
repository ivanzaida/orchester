//! `connections`: the project's connections as Connections lists them (layer `Connections` `mKKPM`): the Codex
//! CLI checked and signed in, which the design's profiles run on; the Claude Code CLI checked but not signed
//! in; GitHub through `gh`, signed in; and three MCP servers: a program that is on, with variables and tools
//! that run without asking and a profile that chooses it (Designer), a program that is turned off, and a
//! remote server with a bearer secret. The programs are empty files in the repository's `.orchester/qa-bin`:
//! what the connections recorded comes from their checks, not from running them. Check again runs them.

use std::{collections::BTreeMap, fs, path::PathBuf};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, GitHubCheck,
    GitHubCheckResult, GitHubConnection, GitHubHost, GitHubLogin, ModelCatalog, SignIn,
    mcp::{
      ExemptTools, HttpServer, McpAuthorization, McpEnvValue, McpEnvironment, McpServer, McpServerName,
      McpServerSelection, McpTransport, McpUrl, StdioServer,
    },
  },
  id::{ConnectionId, SecretId},
  resource::{AgentProfile, CommandArg, CommandProgram, ModelId, PromptText, ProviderSettings, Slug, VariableName},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};

use crate::kontur::{Kontur, SeedError, catalogue, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  seeded(kontur).map(|_| ())
}

/// What [`seed`] made that other scenarios build on.
pub(super) struct Seeded {
  pub codex: ConnectionId,
  pub github: ConnectionId,
}

/// The scenario's connections, MCP servers and profile; the Codex and GitHub connections' ids.
pub(super) fn seeded(kontur: &mut Kontur) -> Result<Seeded, SeedError> {
  let at = kontur.ago(4);
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  fs::create_dir_all(&bin).map_err(|error| SeedError::Invalid(error.to_string()))?;
  let program = |file: &str| -> Result<PathBuf, SeedError> {
    let path = bin.join(file);
    fs::write(&path, "").map_err(|error| SeedError::Invalid(error.to_string()))?;
    Ok(path)
  };
  let codex = cli(
    kontur,
    ("Codex · Kontur", AgentProvider::CodexCli, program("codex")?),
    ("0.46.0", SignIn::SignedIn, &["gpt-5.5", "gpt-5.4-mini"]),
    at,
  )?;
  cli(
    kontur,
    ("Claude · Kontur", AgentProvider::ClaudeCodeCli, program("claude")?),
    (
      "2.1.89",
      SignIn::SignedOut,
      &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"],
    ),
    at,
  )?;
  let published = kontur.ago(7 * 24 * 60);
  catalogue::kontur(&mut kontur.db, codex, published)?;
  let github = github(kontur, program("gh")?, at)?;
  let docs = mcp_servers(kontur, at)?;
  designer(kontur, (codex, docs), at)?;
  Ok(Seeded { codex, github })
}

/// A CLI connection, recorded as its check found it.
pub(super) fn cli(
  kontur: &mut Kontur,
  (named, provider, program): (&str, AgentProvider, PathBuf),
  (version, sign_in, models): (&str, SignIn, &[&str]),
  at: Timestamp,
) -> Result<ConnectionId, SeedError> {
  let mut connection = AgentConnection::new(ConnectionId::generate(), name(named), provider, program, at)?;
  let models = models
    .iter()
    .map(|model| model_id(model).map(|id| CatalogModel::new(id, Vec::new())))
    .collect::<Result<Vec<_>, _>>()?;
  connection.record_check(ConnectionCheck {
    at,
    result: CheckResult::Available {
      version: version.to_owned(),
      sign_in,
      catalog: ModelCatalog::new(CatalogSource::Provider, models),
    },
  })?;
  let connection = connection.into_connection();
  kontur.db.create_connection(&connection)?;
  Ok(connection.id())
}

/// GitHub through `gh` at `program`, signed in as ivanzaida with the `repo` scope; its id.
pub(super) fn github(kontur: &mut Kontur, program: PathBuf, at: Timestamp) -> Result<ConnectionId, SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let mut connection = GitHubConnection::new(
    ConnectionId::generate(),
    name("GitHub · github.com"),
    GitHubHost::github_com(),
    program,
    at,
  )?;
  connection.record_check(GitHubCheck {
    at,
    result: GitHubCheckResult::Available {
      version: "2.62.0".to_owned(),
      account: Some(GitHubLogin::new("ivanzaida").map_err(|error| invalid(&error))?),
      scopes: vec!["repo".to_owned(), "read:org".to_owned()],
      sign_in: SignIn::SignedIn,
    },
  });
  let connection = connection.into_connection();
  kontur.db.create_connection(&connection)?;
  Ok(connection.id())
}

/// kontur-docs (on), postgres-dev (off) and sentry (remote, with a bearer secret); kontur-docs' id.
fn mcp_servers(kontur: &mut Kontur, at: Timestamp) -> Result<ConnectionId, SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let token = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("SENTRY_TOKEN").map_err(|error| invalid(&error))?,
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    at,
  );
  kontur.db.create_secret(&token)?;
  let stdio = |program: &str, args: &[&str]| -> Result<McpTransport, SeedError> {
    let args = args
      .iter()
      .map(|arg| CommandArg::new(arg).map_err(|error| invalid(&error)))
      .collect::<Result<Vec<_>, _>>()?;
    let program = CommandProgram::new(program).map_err(|error| invalid(&error))?;
    Ok(McpTransport::Stdio(StdioServer::new(program, args, None)))
  };
  let index = VariableName::new("DOCS_INDEX").map_err(|error| invalid(&error))?;
  let docs_environment = McpEnvironment::new(BTreeMap::from([(
    index,
    McpEnvValue::Text(".orchester/docs-index".to_owned()),
  )]))
  .map_err(|error| invalid(&error))?;
  let mut docs = server(
    (
      "kontur-docs",
      stdio("npx", &["-y", "@kontur/docs-mcp", "--root", "docs"])?,
    ),
    docs_environment,
    at,
  )?;
  docs.set_exempt_tools(ExemptTools::parse("search_docs\nget_page").map_err(|error| invalid(&error))?);
  let mut postgres = server(
    (
      "postgres-dev",
      stdio("uvx", &["postgres-mcp", "--access-mode", "read-only"])?,
    ),
    McpEnvironment::default(),
    at,
  )?;
  postgres.set_enabled(false);
  let url = McpUrl::new("https://mcp.sentry.dev/mcp").map_err(|error| invalid(&error))?;
  let sentry = server(
    (
      "sentry",
      McpTransport::Http(HttpServer::new(url, McpAuthorization::Bearer { token: token.id() })),
    ),
    McpEnvironment::default(),
    at,
  )?;
  for server in [&docs, &postgres, &sentry] {
    kontur.db.create_mcp_server(server)?;
  }
  Ok(docs.id())
}

fn server(
  (named, transport): (&str, McpTransport),
  environment: McpEnvironment,
  at: Timestamp,
) -> Result<McpServer, SeedError> {
  let name = McpServerName::new(named).map_err(|error| SeedError::Invalid(error.to_string()))?;
  McpServer::new(ConnectionId::generate(), name, transport, environment, at)
    .map_err(|error| SeedError::Invalid(error.to_string()))
}

/// Designer, a profile draft on Codex whose agents get kontur-docs.
fn designer(kontur: &mut Kontur, (codex, docs): (ConnectionId, ConnectionId), at: Timestamp) -> Result<(), SeedError> {
  let servers = McpServerSelection::new(vec![docs]).map_err(|error| SeedError::Invalid(error.to_string()))?;
  let body = AgentProfile::new(
    name("Designer"),
    String::new(),
    PromptText::new("Design the screens.").map_err(|error| SeedError::Invalid(error.to_string()))?,
    codex,
    model_id(catalogue::MODEL)?,
    ProviderSettings::default(),
  )
  .with_mcp_servers(servers);
  let slug = Slug::new("designer").map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.create_resource(slug, name("Designer"), &body, at)?;
  Ok(())
}

fn model_id(id: &str) -> Result<ModelId, SeedError> {
  ModelId::new(id).map_err(|error| SeedError::Invalid(error.to_string()))
}
