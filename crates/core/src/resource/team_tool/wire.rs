//! The JSON of a team tool: the local tool's shape, with each secret a [`SecretName`]. Unknown fields are refused, so
//! a local id, a value or a path has nowhere to go, and an id where a name belongs is not a name.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{TeamDatabaseQuery, TeamHttpRequest, TeamTool, TeamToolAction, TeamToolError};
use crate::{
  resource::{AccessPolicy, CommandAction, FieldName, HttpMethod, RequestTarget, ResponseSource},
  secret::SecretName,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamToolWire {
  #[serde(default, skip_serializing_if = "String::is_empty")]
  description: String,
  action: ActionWire,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  access: Option<AccessPolicy>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActionWire {
  Http(HttpWire),
  Command(CommandAction),
  DatabaseQuery(DatabaseWire),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpWire {
  method: HttpMethod,
  url: String,
  allowed_host: String,
  auth: AuthWire,
  inputs: BTreeMap<FieldName, RequestTarget>,
  outputs: BTreeMap<FieldName, ResponseSource>,
}

/// `"none"` or `{"bearer": {"secret": NAME}}`, as the local tool's auth with a name for its id.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuthWire {
  None,
  Bearer(BearerWire),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BearerWire {
  secret: SecretName,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseWire {
  credentials: SecretName,
  query: String,
}

impl TryFrom<TeamToolWire> for TeamTool {
  type Error = TeamToolError;

  fn try_from(wire: TeamToolWire) -> Result<Self, TeamToolError> {
    let action = match wire.action {
      ActionWire::Command(command) => TeamToolAction::Command(command),
      ActionWire::Http(http) => {
        let bearer = match http.auth {
          AuthWire::None => None,
          AuthWire::Bearer(bearer) => Some(bearer.secret),
        };
        TeamToolAction::Http(TeamHttpRequest::new(
          http.method,
          (http.url, http.allowed_host),
          bearer,
          http.inputs,
          http.outputs,
        )?)
      }
      ActionWire::DatabaseQuery(query) => {
        TeamToolAction::DatabaseQuery(TeamDatabaseQuery::new(query.credentials, query.query)?)
      }
    };
    Ok(Self::new(&wire.description, action, wire.access)?)
  }
}

impl From<TeamTool> for TeamToolWire {
  fn from(tool: TeamTool) -> Self {
    let action = match tool.action {
      TeamToolAction::Command(command) => ActionWire::Command(command),
      TeamToolAction::Http(http) => ActionWire::Http(HttpWire {
        method: http.method(),
        url: http.url().to_owned(),
        allowed_host: http.allowed_host().to_owned(),
        auth: http
          .bearer
          .map_or(AuthWire::None, |secret| AuthWire::Bearer(BearerWire { secret })),
        inputs: http.request.inputs().clone(),
        outputs: http.request.outputs().clone(),
      }),
      TeamToolAction::DatabaseQuery(query) => ActionWire::DatabaseQuery(DatabaseWire {
        credentials: query.credentials,
        query: query.query,
      }),
    };
    Self {
      description: tool.description,
      action,
      access: tool.access,
    }
  }
}
