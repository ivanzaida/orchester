//! `shared-config-here`: Kontur shared with a team server as a member, with what the team's versions name of this
//! computer
//! (`Shared configuration · redesign` `UC5xk`; the data `tests/project_settings/bindings.rs`, `tests/agents/shared.rs`
//! and `tests/pipelines/missing_here.rs` build). The team's cached versions ([`resources`]) name seven references:
//! - connections: Claude Code `Work` (Builder v4, Planner v2), bound by name to *Claude Code · Work*; Codex `Default`
//!   (Reviewer v4), bound by role to *Codex · Personal*; the API `DeepSeek` (Explorer v1), missing, while *DeepSeek*
//!   and *OpenRouter* of its role are here to choose;
//! - MCP servers: `github` (Reviewer v5), found here by name as the remote server `github` (MCP server names are lower
//!   case) and waiting for *Confirm…* (04d); `linear` (Triage v2), missing;
//! - secrets: `STAGING_DB_URL` (tool Query staging), bound by the person; `SENTRY_TOKEN` (tool Report crash), missing,
//!   with `SENTRY_AUTH_TOKEN` here to give it (04e).
//!
//! Reviewer's v5 widens what agents may do (the MCP server `github`, network) and waits for *Use v5 here* (02d); the
//! pipeline Feature delivery runs Explorer, so it can't run here (04). Builder's page shows *Runs here use* (06, 06b).
//! The pickers and the modals open with a click; only choosing in them and confirming are a person's.
//!
//! `shared-config-chosen` is it with `Work` chosen here by the person (07); `shared-config-no-claude` has no Claude
//! Code connection at all, so `Work` is missing with *Add an account…* and Builder can't run here (07c, 06c).

mod resources;

use std::{collections::HashMap, fs, path::PathBuf};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, EndpointPreset, SignIn,
    mcp::{HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpTransport, McpUrl},
  },
  id::{ConnectionId, ProjectId, SecretId, ServerId, UserId},
  project::ProjectKey,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  sharing::MemberRole,
  store::{ConnectionStore, McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{Bound, BoundBy, BoundReference, ReferenceBinding, SharedMember, SharingLink};

use super::{api_providers::key_secret, connections};
use crate::kontur::{Kontur, SeedError, name};

const DAY: i64 = 24 * 60;

/// How the Claude Code reference `Work` stands here.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
  /// Bound by its name to *Claude Code · Work*.
  ByName,
  /// Chosen here by the person.
  Chosen,
  /// This computer has no Claude Code connection.
  NoClaudeCode,
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared(kontur, Work::ByName)
}

pub fn chosen(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared(kontur, Work::Chosen)
}

pub fn no_claude(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared(kontur, Work::NoClaudeCode)
}

/// The publisher's ids of what the team's versions name.
struct Theirs {
  work: ConnectionId,
  default: ConnectionId,
  deepseek: ConnectionId,
  github: ConnectionId,
  linear: ConnectionId,
  staging: SecretId,
  sentry: SecretId,
}

impl Theirs {
  fn generate() -> Self {
    Self {
      work: ConnectionId::generate(),
      default: ConnectionId::generate(),
      deepseek: ConnectionId::generate(),
      github: ConnectionId::generate(),
      linear: ConnectionId::generate(),
      staging: SecretId::generate(),
      sentry: SecretId::generate(),
    }
  }
}

/// What this computer has that the references are bound to.
struct Local {
  /// *Claude Code · Work*; `None` without Claude Code.
  work: Option<ConnectionId>,
  codex: ConnectionId,
  staging: SecretId,
}

fn shared(kontur: &mut Kontur, work: Work) -> Result<(), SeedError> {
  let theirs = Theirs::generate();
  let local = local(kontur, work)?;
  link(kontur)?;
  resources::keep(kontur, &theirs, &local)?;
  bind(kontur, &theirs, &local, work)
}

/// The person's connections, the MCP server `github` and the secrets on this computer.
fn local(kontur: &mut Kontur, work: Work) -> Result<Local, SeedError> {
  let at = kontur.ago(3 * DAY);
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  fs::create_dir_all(&bin).map_err(invalid)?;
  let program = |file: &str| -> Result<PathBuf, SeedError> {
    let path = bin.join(file);
    fs::write(&path, "").map_err(invalid)?;
    Ok(path)
  };
  let claude = (
    "2.1.173",
    SignIn::SignedIn,
    &["claude-opus-5-5", "claude-sonnet-5-5"][..],
  );
  let work = match work {
    Work::NoClaudeCode => None,
    Work::ByName | Work::Chosen => {
      connections::cli(
        kontur,
        (
          "Claude Code · Default",
          AgentProvider::ClaudeCodeCli,
          program("claude")?,
        ),
        claude,
        at,
      )?;
      Some(connections::cli(
        kontur,
        ("Claude Code · Work", AgentProvider::ClaudeCodeCli, program("claude")?),
        claude,
        at,
      )?)
    }
  };
  let codex = connections::cli(
    kontur,
    ("Codex · Personal", AgentProvider::CodexCli, program("codex")?),
    ("0.46.0", SignIn::SignedIn, &["gpt-5.5"]),
    at,
  )?;
  api(
    kontur,
    ("DeepSeek", "DEEPSEEK_API_KEY"),
    EndpointPreset::DeepSeek,
    "https://api.deepseek.com",
  )?;
  api(
    kontur,
    ("OpenRouter", "OPENROUTER_API_KEY"),
    EndpointPreset::Custom,
    "https://openrouter.ai/api/v1",
  )?;
  github(kontur, at)?;
  let staging = secret(
    kontur,
    "STAGING_DB_URL",
    SecretKind::DatabaseConnection,
    Environment::Staging,
  )?;
  secret(
    kontur,
    "SENTRY_AUTH_TOKEN",
    SecretKind::AccessToken,
    Environment::Development,
  )?;
  Ok(Local { work, codex, staging })
}

/// A native connection on the OpenAI API named `named`, its key in the secret `key`.
fn api(kontur: &mut Kontur, (named, key): (&str, &str), preset: EndpointPreset, url: &str) -> Result<(), SeedError> {
  let key = key_secret(kontur, key, None, kontur.ago(4 * DAY))?;
  let endpoint = ApiEndpoint::new(preset, BaseUrl::new(url).map_err(invalid)?, key);
  let connection = AgentConnection::native(
    ConnectionId::generate(),
    name(named),
    AgentProvider::OpenAiApi,
    endpoint,
    kontur.ago(4 * DAY),
  )?;
  kontur.db.create_connection(&connection.into_connection())?;
  Ok(())
}

/// `github`, a remote MCP server here whose name matches the team's `github`.
fn github(kontur: &mut Kontur, at: Timestamp) -> Result<(), SeedError> {
  let url = McpUrl::new("https://api.githubcopilot.com/mcp").map_err(invalid)?;
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("github").map_err(invalid)?,
    McpTransport::Http(HttpServer::new(url, McpAuthorization::None)),
    McpEnvironment::default(),
    at,
  )
  .map_err(invalid)?;
  kontur.db.create_mcp_server(&server)?;
  Ok(())
}

/// A secret record named `named`; no value is in any keychain.
fn secret(kontur: &mut Kontur, named: &str, kind: SecretKind, environment: Environment) -> Result<SecretId, SeedError> {
  let new = NewSecret {
    name: SecretName::new(named).map_err(invalid)?,
    kind,
    environment,
  };
  let secret = Secret::new(SecretId::generate(), new, kontur.ago(10 * DAY));
  kontur.db.create_secret(&secret)?;
  Ok(secret.id())
}

/// Links Kontur to a team server project as a member; dana-k owns it.
fn link(kontur: &mut Kontur) -> Result<(), SeedError> {
  let me = UserId::generate();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").map_err(invalid)?,
    role: Some(MemberRole::Member),
    joined_at: kontur.ago(20 * DAY),
    cursor: 0,
    last_synced_at: kontur.ago(5),
    removed_at: None,
    address: Some("https://orchester.example.com/kontur".to_owned()),
  };
  kontur.db.start_sharing(&link, &[], (&[], &HashMap::new()))?;
  let added = kontur.ago(20 * DAY);
  let member = |user, (display, username, role): (&str, &str, MemberRole)| SharedMember {
    user,
    display_name: display.to_owned(),
    username: username.to_owned(),
    role,
    added_at: added,
  };
  let members = [
    member(UserId::generate(), ("Dana Kovalenko", "dana-k", MemberRole::Owner)),
    member(UserId::generate(), ("Olha", "olha", MemberRole::Member)),
    member(me, ("Ivan", "ivan", MemberRole::Member)),
  ];
  kontur.db.record_shared_members(&members, kontur.ago(5), Some(me))?;
  Ok(())
}

/// How this computer keeps each reference.
fn bind(kontur: &mut Kontur, theirs: &Theirs, local: &Local, work: Work) -> Result<(), SeedError> {
  let by = |local, by| Some(Bound { local, by });
  let work = local.work.and_then(|connection| match work {
    Work::ByName => by(connection, BoundBy::Name),
    Work::Chosen => by(connection, BoundBy::Person),
    Work::NoClaudeCode => None,
  });
  let rows = [
    (
      BoundReference::Connection {
        reference: theirs.work,
        bound: work,
      },
      "Work",
      Some("claude_code_cli"),
    ),
    (
      BoundReference::Connection {
        reference: theirs.default,
        bound: by(local.codex, BoundBy::Role),
      },
      "Default",
      Some("codex_cli"),
    ),
    (
      BoundReference::Connection {
        reference: theirs.deepseek,
        bound: None,
      },
      "DeepSeek",
      Some("openai_api"),
    ),
    (
      BoundReference::McpServer {
        reference: theirs.github,
        bound: None,
      },
      "github",
      None,
    ),
    (
      BoundReference::McpServer {
        reference: theirs.linear,
        bound: None,
      },
      "linear",
      None,
    ),
    (
      BoundReference::Secret {
        reference: theirs.staging,
        bound: Some(Bound {
          local: local.staging,
          by: BoundBy::Person,
        }),
      },
      "STAGING_DB_URL",
      Some("database_connection"),
    ),
    (
      BoundReference::Secret {
        reference: theirs.sentry,
        bound: None,
      },
      "SENTRY_TOKEN",
      Some("access_token"),
    ),
  ];
  let changed_at = kontur.ago(3 * DAY);
  for (reference, named, role) in rows {
    kontur.db.set_reference_binding(&ReferenceBinding {
      reference,
      name: named.to_owned(),
      role: role.map(str::to_owned),
      changed_at,
    })?;
  }
  Ok(())
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
