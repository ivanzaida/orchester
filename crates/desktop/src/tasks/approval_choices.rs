//! What the approval dialog's wider answers cover for one request ("Approval choices and waiting
//! requests"), read with the request: what Approve similar would make a rule of, what Always allow would
//! add and where, and who that affects. Whether each is available is core's decision
//! (`orchester_core::run`); this only gathers the names the dialog shows.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::mcp::McpToolName,
  id::{ConnectionId, ResourceId},
  permission::{
    AgentPermissions, CommandPrefix,
    category::{CommandCategory, CommandScope},
  },
  resource::{AgentProfile, Pipeline, ResourceKind, StepKind, VersionNumber},
  run::{
    AlwaysUnavailable, ApprovalSubject, PendingToolApproval, RuleScope, SimilarUnavailable, always_prefixes,
    codex_denial, denial_message,
  },
  store::{McpServerStore, ResourceStore},
};
use orchester_store::ProjectDb;

/// What Approve similar would make a rule of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimilarShown {
  /// `cargo add` commands in a category.
  Command {
    words: Arc<str>,
    category: CommandCategory,
  },
  Tool {
    server: Arc<str>,
    tool: Arc<str>,
  },
}

/// A prefix for a profile's Allowed commands, published as `next` (the run started on `pinned`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandAlways {
  pub prefixes: Vec<CommandPrefix>,
  pub profile: Arc<str>,
  pub pinned: Option<VersionNumber>,
  pub next: VersionNumber,
  /// The profile has an unsaved draft, which gets the prefix too.
  pub draft: bool,
  /// The published pipelines with a step of the profile.
  pub pipelines: Vec<Arc<str>>,
}

/// What Always allow would add, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlwaysShown {
  Command(CommandAlways),
  /// The tool, for the server's Tools that run without asking.
  Tool {
    server: Arc<str>,
    tool: Arc<str>,
    /// The profiles that get the server, and those that do not.
    profiles: Vec<Arc<str>>,
    others: Vec<Arc<str>>,
  },
}

/// Why Always allow is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlwaysBlocked {
  Command(AlwaysUnavailable),
  /// The tool's name is not one a server can list.
  ToolName,
  /// The step's profile or the server cannot be read any more.
  Missing,
}

/// The wider answers of one request, and what Deny tells the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalChoices {
  pub similar: Result<SimilarShown, SimilarUnavailable>,
  pub always: Result<AlwaysShown, AlwaysBlocked>,
  /// What a denial tells the agent: Orchester's words for Claude Code, Codex's own for Codex.
  pub told: Arc<str>,
  /// Codex asks: it declines a denied request and tells the agent in its own words.
  pub codex: bool,
  /// A command's category; `None` for an MCP call.
  pub category: Option<CommandCategory>,
}

impl ApprovalChoices {
  /// The choices of `approval`, made in step profile `profile` (pinned at `pinned`) whose agent works in
  /// `worktree` and may also write to `writable`; `codex` when Codex asks. A command is classified as the
  /// runner classifies it: in that worktree, from the working folder the request reports.
  pub fn read(
    db: &ProjectDb,
    approval: &PendingToolApproval,
    (profile, pinned): (Option<ResourceId>, Option<VersionNumber>),
    (worktree, writable, codex): (&Path, &[PathBuf], bool),
  ) -> Self {
    let subject = ApprovalSubject::of(approval, &CommandScope::on_host(worktree, writable));
    let similar = RuleScope::similar(&subject).and_then(|scope| match scope {
      RuleScope::SimilarCommand { command } => Ok(SimilarShown::Command {
        words: command.words().into(),
        category: command.category(),
      }),
      RuleScope::SimilarMcpTool { server, tool } => Ok(SimilarShown::Tool {
        server: server.into(),
        tool: tool.into(),
      }),
      // Approve similar never makes a rule of Always allow.
      RuleScope::AlwaysCommand { .. } => Err(SimilarUnavailable::Unknown),
    });
    let always = match &subject {
      ApprovalSubject::Command(command) => profile
        .and_then(|profile| ProfileState::read(db, profile))
        .ok_or(AlwaysBlocked::Missing)
        .and_then(|state| {
          let prefixes = always_prefixes(command, state.permissions.commands()).map_err(AlwaysBlocked::Command)?;
          let next = state.latest.next().ok_or(AlwaysBlocked::Missing)?;
          Ok(AlwaysShown::Command(CommandAlways {
            prefixes,
            profile: state.name,
            pinned,
            next,
            draft: state.draft,
            pipelines: pipelines_of(db, state.id),
          }))
        }),
      ApprovalSubject::McpTool { server, tool } => tool_choice(db, server, tool),
    };
    Self {
      similar,
      always,
      told: if codex {
        codex_denial(approval.command).into()
      } else {
        denial_message(approval).into()
      },
      codex,
      category: match &subject {
        ApprovalSubject::Command(command) => Some(command.category()),
        ApprovalSubject::McpTool { .. } => None,
      },
    }
  }
}

/// A profile as Always allow of a command adds to it: its latest version and whether it has a draft.
struct ProfileState {
  id: ResourceId,
  name: Arc<str>,
  latest: VersionNumber,
  permissions: AgentPermissions,
  draft: bool,
}

impl ProfileState {
  fn read(db: &ProjectDb, id: ResourceId) -> Option<Self> {
    // `.ok()`: an unreadable profile only makes Always allow unavailable; the request stays decidable.
    let latest = db.latest_version::<AgentProfile>(id).ok()??;
    let draft = db.draft::<AgentProfile>(id).ok()?.is_some();
    Some(Self {
      id,
      name: latest.body().role().as_str().into(),
      latest: latest.number(),
      permissions: latest.body().permissions().clone(),
      draft,
    })
  }
}

/// The published pipelines with a step of `profile`, by name.
fn pipelines_of(db: &ProjectDb, profile: ResourceId) -> Vec<Arc<str>> {
  // `.ok()`: an unreadable list only leaves the names out of the note.
  let resources = db.resources(ResourceKind::Pipeline).ok().unwrap_or_default();
  resources
    .into_iter()
    .filter(|resource| {
      db.latest_version::<Pipeline>(resource.id())
        .ok()
        .flatten()
        .is_some_and(|version| {
          version
            .body()
            .steps()
            .iter()
            .any(|step| matches!(step.kind, StepKind::Agent { profile: Some(id), .. } if id == profile))
        })
    })
    .map(|resource| resource.name().as_str().into())
    .collect()
}

/// Always allow of `tool` of `server`: the server, and the profiles that get it and those that do not.
fn tool_choice(db: &ProjectDb, server: &str, tool: &str) -> Result<AlwaysShown, AlwaysBlocked> {
  if McpToolName::new(tool).is_err() {
    return Err(AlwaysBlocked::ToolName);
  }
  // `.ok()`: an unreadable server list makes Always allow unavailable, not the request.
  let servers = db.mcp_servers().ok().unwrap_or_default();
  let id = servers
    .iter()
    .find(|stored| stored.name().as_str() == server)
    .map(|stored| stored.id())
    .ok_or(AlwaysBlocked::Missing)?;
  let (profiles, others) = profiles_by_server(db, id);
  Ok(AlwaysShown::Tool {
    server: server.into(),
    tool: tool.into(),
    profiles,
    others,
  })
}

/// The agent profiles whose latest version gets server `id`, and the others, by name.
fn profiles_by_server(db: &ProjectDb, id: ConnectionId) -> (Vec<Arc<str>>, Vec<Arc<str>>) {
  let (mut getting, mut others) = (Vec::new(), Vec::new());
  // `.ok()`: an unreadable profile is left out of the names.
  for resource in db.resources(ResourceKind::AgentProfile).ok().unwrap_or_default() {
    let Some(latest) = db.latest_version::<AgentProfile>(resource.id()).ok().flatten() else {
      continue;
    };
    let name: Arc<str> = latest.body().role().as_str().into();
    if latest.body().mcp_servers().contains(id) {
      getting.push(name);
    } else {
      others.push(name);
    }
  }
  (getting, others)
}
