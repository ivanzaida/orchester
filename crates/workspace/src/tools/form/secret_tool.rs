//! An HTTP or database tool as typed: what it does, its request or query, the secret it sends or connects with, and its
//! access, which such a tool needs (A32–A35, L20–L24). A local project's tool holds the secret it picked from the
//! project's by id; a team's names it by name, and each computer fills in the value.

use orchester_core::{
  id::SecretId,
  resource::{
    AccessPolicy, DatabaseQuery, HttpAuth, TeamDatabaseQuery, TeamTool, TeamToolAction, Tool, ToolAction, ToolError,
  },
  secret::{SecretName, SecretNameError},
};

use super::{
  access::{AccessErrors, AccessText},
  http::{HttpErrors, HttpText},
};
use crate::messages::Message;

/// What the tool runs, as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionText {
  Http(HttpText),
  /// The query, run as written.
  Database(String),
}

/// The fields as the user left them, but for the secret, which the local form picks and the team's names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretToolText {
  pub description: String,
  pub action: ActionText,
  pub access: AccessText,
}

/// Why the tool cannot be made yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecretToolErrors {
  pub http: HttpErrors,
  /// The secret the request sends or the query connects with.
  pub secret: Option<Message>,
  pub query: Option<Message>,
  pub access: AccessErrors,
}

impl SecretToolText {
  /// The fields of the local `tool` and the secret it holds; `None` for a command.
  pub fn of(tool: &Tool) -> Option<(Self, Option<SecretId>)> {
    let action = match tool.action() {
      ToolAction::Http(request) => ActionText::Http(HttpText::of(request)),
      ToolAction::DatabaseQuery(query) => ActionText::Database(query.query().to_owned()),
      ToolAction::Command(_) => return None,
    };
    Some((Self::with(tool.description(), action, tool.access()), tool.secret()))
  }

  /// The fields of the team's `tool` and the name of the secret it holds; `None` for a command.
  pub fn of_team(tool: &TeamTool) -> Option<(Self, Option<SecretName>)> {
    let action = match tool.action() {
      TeamToolAction::Http(request) => ActionText::Http(HttpText::of_team(request)),
      TeamToolAction::DatabaseQuery(query) => ActionText::Database(query.query().to_owned()),
      TeamToolAction::Command(_) => return None,
    };
    Some((
      Self::with(tool.description(), action, tool.access()),
      tool.secret().cloned(),
    ))
  }

  fn with(description: &str, action: ActionText, access: Option<&AccessPolicy>) -> Self {
    Self {
      description: description.to_owned(),
      action,
      access: access.map(AccessText::of).unwrap_or_default(),
    }
  }

  /// Whether the tool sends or connects with a secret: a database query always does, a request with a bearer token.
  pub fn takes_secret(&self) -> bool {
    match &self.action {
      ActionText::Http(http) => http.bearer,
      ActionText::Database(_) => true,
    }
  }

  /// The local tool, with the project's secret `secret` it picked.
  pub fn tool(&self, secret: Option<SecretId>) -> Result<Tool, Box<SecretToolErrors>> {
    let secret = self
      .takes_secret()
      .then(|| secret.ok_or_else(|| Message::new("tools.errors.secretRequired")))
      .transpose();
    let mut errors = SecretToolErrors::default();
    let action = match &self.action {
      ActionText::Http(http) => match (http.checked(), secret) {
        (Ok(checked), Ok(secret)) => checked
          .request(secret.map_or(HttpAuth::None, |secret| HttpAuth::Bearer { secret }))
          .map(ToolAction::Http)
          .map_err(|http| errors.http = *http),
        (checked, secret) => {
          errors.http = checked.err().map(|http| *http).unwrap_or_default();
          errors.secret = secret.err();
          Err(())
        }
      },
      ActionText::Database(query) => match (secret, database(query)) {
        (Ok(Some(secret)), Ok(query)) => Ok(ToolAction::DatabaseQuery(
          DatabaseQuery::new(secret, query).expect("the query is not blank"),
        )),
        (secret, query) => {
          errors.secret = secret.err();
          errors.query = query.err();
          Err(())
        }
      },
    };
    self.finish(action, errors, |action, access| {
      Tool::new(&self.description, action, Some(access))
    })
  }

  /// The team's tool, with the team secret named `secret`.
  pub fn team_tool(&self, secret: &str) -> Result<TeamTool, Box<SecretToolErrors>> {
    let secret = self.takes_secret().then(|| named_secret(secret)).transpose();
    let mut errors = SecretToolErrors::default();
    let action = match &self.action {
      ActionText::Http(http) => match (http.checked(), secret) {
        (Ok(checked), Ok(bearer)) => checked
          .team_request(bearer)
          .map(TeamToolAction::Http)
          .map_err(|http| errors.http = *http),
        (checked, secret) => {
          errors.http = checked.err().map(|http| *http).unwrap_or_default();
          errors.secret = secret.err();
          Err(())
        }
      },
      ActionText::Database(query) => match (secret, database(query)) {
        (Ok(Some(secret)), Ok(query)) => Ok(TeamToolAction::DatabaseQuery(
          TeamDatabaseQuery::new(secret, query).expect("the query is not blank"),
        )),
        (secret, query) => {
          errors.secret = secret.err();
          errors.query = query.err();
          Err(())
        }
      },
    };
    self.finish(action, errors, |action, access| {
      TeamTool::new(&self.description, action, Some(access))
    })
  }

  /// The tool `make` makes of `action` and the access policy, once both are made; else every error found.
  fn finish<A, T>(
    &self,
    action: Result<A, ()>,
    mut errors: SecretToolErrors,
    make: impl FnOnce(A, AccessPolicy) -> Result<T, ToolError>,
  ) -> Result<T, Box<SecretToolErrors>> {
    let access = self.access.policy().map_err(|access| errors.access = access);
    let (Ok(action), Ok(access)) = (action, access) else {
      return Err(Box::new(errors));
    };
    Ok(make(action, access).expect("a tool with an access policy is a tool"))
  }
}

/// The query as typed, which must not be blank.
fn database(query: &str) -> Result<String, Message> {
  if query.trim().is_empty() {
    Err(Message::new("tools.errors.queryRequired"))
  } else {
    Ok(query.to_owned())
  }
}

/// The team secret named `text`.
pub fn named_secret(text: &str) -> Result<SecretName, Message> {
  SecretName::new(text).map_err(|error| match error {
    SecretNameError::Blank => Message::new("tools.errors.secretNameRequired"),
    SecretNameError::TooLong { .. } => {
      Message::new("tools.errors.secretNameTooLong").with("max", SecretName::MAX_CHARS.to_string())
    }
    SecretNameError::Invalid => Message::new("tools.errors.secretName"),
  })
}
