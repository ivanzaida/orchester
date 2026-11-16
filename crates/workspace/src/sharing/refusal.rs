//! What Share cannot upload as the team's, collected over the whole project so the person sees every resource to fix
//! at once (`docs/design/team-configuration.md`, edge case 12). Share refuses rather than drops: a resource whose body
//! names something of this computer that the team form has no place for, or a local MCP server that cannot be a team
//! MCP server, stops the whole Share before anything is sent.

use orchester_core::{id::ResourceId, resource::TeamMcpServerError};

use crate::{messages::Message, sharing::SharingError};

/// One resource Share does not upload, by its name, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareRefusal {
  /// The resource's id; for a local MCP server, the id the team MCP server would have.
  pub resource: ResourceId,
  pub name: String,
  pub reason: ShareRefusalReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareRefusalReason {
  /// The body names connections, MCP servers or secrets of this computer (`names`) that its kind has no team form
  /// for, or an MCP server a profile chooses that this computer no longer has.
  LocalEntities { names: Vec<String> },
  /// A pipeline runs a profile or tool, or merges with an integrator, that is not uploaded (`name`).
  PipelineNamesNonTeam { name: String },
  /// A profile has writable folders, which are paths of this computer.
  WritableFolders { folders: Vec<String> },
  /// A tool uses a secret the project does not have, so its name cannot be given to the team.
  ToolSecretGone,
  /// A local MCP server (`server`) cannot be a team MCP server.
  McpServer { server: String, problem: TeamMcpProblem },
}

impl ShareRefusalReason {
  /// The reason `error`, from turning a resource into the team's form, refuses it; any other error is not a refusal
  /// and comes back to stop the Share.
  pub fn of(error: SharingError) -> Result<Self, SharingError> {
    match error {
      SharingError::LocalEntitiesNamed { names, .. } => Ok(Self::LocalEntities { names }),
      SharingError::PipelineNamesNonTeam { name, .. } => Ok(Self::PipelineNamesNonTeam { name }),
      SharingError::WritableFoldersStay { folders, .. } => Ok(Self::WritableFolders { folders }),
      SharingError::ToolSecretGone { .. } => Ok(Self::ToolSecretGone),
      other => Err(other),
    }
  }
}

/// Why a local MCP server cannot be a team MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TeamMcpProblem {
  /// The program is a path inside the worktree, which the team's body cannot name (it names a program by its name on
  /// the search path).
  ProgramInWorktree { program: String },
  /// The program's file name is not a name to look up on the search path.
  ProgramName { program: String },
  /// The working directory is absolute; the team's is relative to the worktree.
  AbsoluteFolder { folder: String },
  /// The URL has a query or a fragment, where a key would reach every member; a secret belongs in a header.
  UrlQuery,
  /// Basic authentication with a user name: the team's secret header holds a whole credential, not a user name.
  BasicUsername,
  /// A variable's name is not a valid secret name; under the team's form each variable takes the team secret of its
  /// own name.
  VariableNotSecret { variable: String },
  /// A secret header names no secret.
  HeaderWithoutSecret { header: String },
  /// The prefix of a secret header is not a scheme (letters, spaces and `- _ . :` only).
  HeaderPrefix,
  /// A team secret (`secret`) is filled from another local secret or value for the MCP server `other`: a team secret
  /// has one value on a computer.
  SecretTaken { secret: String, other: String },
  /// Any other rule of the team's body, as its error says (it holds names only, never a value).
  Body { detail: String },
}

impl From<TeamMcpServerError> for TeamMcpProblem {
  fn from(error: TeamMcpServerError) -> Self {
    match error {
      TeamMcpServerError::ProgramPath(program) => Self::ProgramName { program },
      TeamMcpServerError::AbsoluteFolder(folder) => Self::AbsoluteFolder { folder },
      TeamMcpServerError::UrlQuery => Self::UrlQuery,
      TeamMcpServerError::VariableNotSecret(variable) => Self::VariableNotSecret { variable },
      TeamMcpServerError::HeaderWithoutSecret(header) => Self::HeaderWithoutSecret { header },
      TeamMcpServerError::Prefix(_) => Self::HeaderPrefix,
      other => Self::Body {
        detail: other.to_string(),
      },
    }
  }
}

impl ShareRefusal {
  /// What to tell the person about this resource.
  pub fn message(&self) -> Message {
    let resource = self.name.clone();
    match &self.reason {
      ShareRefusalReason::LocalEntities { names } => {
        Message::new("sharing.errors.shareRefusal.localEntities").with("names", names.join(", "))
      }
      ShareRefusalReason::PipelineNamesNonTeam { name } => {
        Message::new("sharing.errors.shareRefusal.pipelineNamesNonTeam").with("name", name.clone())
      }
      ShareRefusalReason::WritableFolders { folders } => {
        Message::new("sharing.errors.shareRefusal.writableFolders").with("folders", folders.join(", "))
      }
      ShareRefusalReason::ToolSecretGone => Message::new("sharing.errors.shareRefusal.toolSecretGone"),
      ShareRefusalReason::McpServer { problem, .. } => mcp_message(problem),
    }
    .with("resource", resource)
  }
}

fn mcp_message(problem: &TeamMcpProblem) -> Message {
  let message = Message::new(match problem {
    TeamMcpProblem::ProgramInWorktree { .. } => "sharing.errors.shareRefusal.mcpProgramInWorktree",
    TeamMcpProblem::ProgramName { .. } => "sharing.errors.shareRefusal.mcpProgramName",
    TeamMcpProblem::AbsoluteFolder { .. } => "sharing.errors.shareRefusal.mcpFolder",
    TeamMcpProblem::UrlQuery => "sharing.errors.shareRefusal.mcpUrlQuery",
    TeamMcpProblem::BasicUsername => "sharing.errors.shareRefusal.mcpBasicUsername",
    TeamMcpProblem::VariableNotSecret { .. } => "sharing.errors.shareRefusal.mcpVariableNotSecret",
    TeamMcpProblem::HeaderWithoutSecret { .. } => "sharing.errors.shareRefusal.mcpHeaderWithoutSecret",
    TeamMcpProblem::HeaderPrefix => "sharing.errors.shareRefusal.mcpHeaderPrefix",
    TeamMcpProblem::SecretTaken { .. } => "sharing.errors.shareRefusal.mcpSecretTaken",
    TeamMcpProblem::Body { .. } => "sharing.errors.shareRefusal.mcpBody",
  });
  match problem {
    TeamMcpProblem::VariableNotSecret { variable } => message.with("variable", variable.clone()),
    TeamMcpProblem::HeaderWithoutSecret { header } => message.with("header", header.clone()),
    TeamMcpProblem::SecretTaken { secret, other } => {
      message.with("secret", secret.clone()).with("other", other.clone())
    }
    _ => message,
  }
}
