//! One local MCP server in the team's form (`docs/design/team-configuration.md`, edge cases 1 and 12): the program by
//! name with no path, each variable and secret header naming the team secret it takes, and what this computer fills in
//! for it. A server that cannot be a team one says why ([`TeamMcpProblem`]).

use std::{fmt, path::PathBuf};

use orchester_core::{
  connection::mcp::{McpAuthorization, McpEnvValue, McpHeaderName, McpServer, McpTransport},
  id::SecretId,
  resource::{
    CommandProgram, HeaderPrefix, ProgramLocation, TeamMcpLaunch, TeamMcpProgram, TeamMcpRemote, TeamMcpSecretHeader,
    TeamMcpServer, TeamMcpServerError,
  },
  secret::{Secret, SecretName},
};

use crate::sharing::TeamMcpProblem;

/// A plain-text value of a variable. Under the team's form it is the value of a team secret, so it is kept out of
/// `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct PlainValue(String);

impl PlainValue {
  pub fn expose(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for PlainValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("PlainValue(<redacted>)")
  }
}

/// Where the value of a team secret comes from on this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FillSource {
  /// A project secret kept in the keychain. `basic` marks a pasted Basic credential, whose team form is the base64
  /// part after the prefix `Basic `.
  Local { secret: Secret, basic: bool },
  /// The plain text a variable had.
  Text(PlainValue),
}

/// The team secret `name` this computer gives a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretFill {
  pub name: SecretName,
  pub source: FillSource,
}

/// How the local server started its program, for this computer's path fill-in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalProgram {
  /// A remote server starts none.
  None,
  /// The absolute file it was started by.
  Path(PathBuf),
  /// A name looked up on the search path, to be resolved here when the Share reads its fill-ins.
  Name(CommandProgram),
}

/// The team's form of a local server and what this computer fills in for it.
#[derive(Debug)]
pub struct TeamForm {
  pub body: TeamMcpServer,
  pub program: LocalProgram,
  pub fills: Vec<SecretFill>,
}

/// Why a local server has no team form.
pub enum FormError {
  Refused(TeamMcpProblem),
  /// The server names a secret this project does not have.
  SecretMissing(SecretId),
}

impl From<TeamMcpProblem> for FormError {
  fn from(problem: TeamMcpProblem) -> Self {
    Self::Refused(problem)
  }
}

impl From<TeamMcpServerError> for FormError {
  fn from(error: TeamMcpServerError) -> Self {
    Self::Refused(error.into())
  }
}

/// The team's form of `server`; `secrets` are the project's.
pub fn team_form(server: &McpServer, secrets: &[Secret]) -> Result<TeamForm, FormError> {
  let secret = |id: SecretId| {
    secrets
      .iter()
      .find(|secret| secret.id() == id)
      .cloned()
      .ok_or(FormError::SecretMissing(id))
  };
  let (launch, program, fills) = match server.transport() {
    McpTransport::Stdio(stdio) => {
      let (named, program) = program_name(stdio.program())?;
      let mut variables = Vec::new();
      let mut fills = Vec::new();
      for (variable, value) in server.environment().iter() {
        let name = SecretName::new(variable.as_str()).map_err(|_| TeamMcpProblem::VariableNotSecret {
          variable: variable.as_str().to_owned(),
        })?;
        let source = match value {
          McpEnvValue::Secret(id) => FillSource::Local {
            secret: secret(*id)?,
            basic: false,
          },
          McpEnvValue::Text(text) => FillSource::Text(PlainValue(text.clone())),
        };
        variables.push(variable.clone());
        fills.push(SecretFill { name, source });
      }
      let launch = TeamMcpProgram::new(
        named,
        stdio.args().to_vec(),
        stdio.working_directory().cloned(),
        variables,
      )?;
      (TeamMcpLaunch::Program(launch), program, fills)
    }
    McpTransport::Http(http) => {
      let (headers, fills): (Vec<_>, Vec<_>) = secret_headers(http.authorization(), secret)?.into_iter().unzip();
      let remote = TeamMcpRemote::new(http.url().clone(), headers)?;
      (TeamMcpLaunch::Remote(remote), LocalProgram::None, fills)
    }
  };
  Ok(TeamForm {
    body: TeamMcpServer::new(server.name().clone(), launch, server.exempt_tools().clone()),
    program,
    fills,
  })
}

/// The program the team names, and how it started here: a name on the search path stays as it is, an absolute path
/// becomes its file name and is kept as this computer's path.
fn program_name(program: &CommandProgram) -> Result<(CommandProgram, LocalProgram), TeamMcpProblem> {
  match program.location() {
    ProgramLocation::OnPath(_) => Ok((program.clone(), LocalProgram::Name(program.clone()))),
    ProgramLocation::InWorktree(_) => Err(TeamMcpProblem::ProgramInWorktree {
      program: program.as_str().to_owned(),
    }),
    ProgramLocation::Absolute(path) => {
      let unusable = || TeamMcpProblem::ProgramName {
        program: program.as_str().to_owned(),
      };
      let file = path.file_name().and_then(|file| file.to_str()).ok_or_else(unusable)?;
      let named = CommandProgram::new(file).map_err(|_| unusable())?;
      if matches!(named.location(), ProgramLocation::OnPath(_)) {
        Ok((named, LocalProgram::Path(path.to_path_buf())))
      } else {
        Err(unusable())
      }
    }
  }
}

/// The headers a remote server is sent a secret in, each with what fills its secret. The team secret is named as the
/// local secret is.
fn secret_headers(
  authorization: &McpAuthorization,
  secret: impl Fn(SecretId) -> Result<Secret, FormError>,
) -> Result<Vec<(TeamMcpSecretHeader, SecretFill)>, FormError> {
  let header = |id, name, scheme: Option<&str>, basic| {
    let local = secret(id)?;
    let prefix = scheme
      .map(HeaderPrefix::new)
      .transpose()
      .map_err(TeamMcpServerError::from)?;
    let team = local.name().clone();
    let fill = SecretFill {
      name: team.clone(),
      source: FillSource::Local { secret: local, basic },
    };
    Ok(vec![(TeamMcpSecretHeader::new(name, team, prefix), fill)])
  };
  match authorization {
    McpAuthorization::None => Ok(Vec::new()),
    McpAuthorization::Bearer { token } => header(*token, McpHeaderName::authorization(), Some("Bearer "), false),
    McpAuthorization::BasicCredential { credential } => {
      header(*credential, McpHeaderName::authorization(), Some("Basic "), true)
    }
    McpAuthorization::Header { name, value } => header(*value, name.clone(), None, false),
    McpAuthorization::Basic { .. } => Err(TeamMcpProblem::BasicUsername.into()),
  }
}
