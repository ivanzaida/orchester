//! A remote server's authorization as the page's fields hold it: the scheme chosen, the one project secret it sends
//! (the bearer token, the Basic password or pasted credential, or the custom header's value), the Basic user name and
//! the custom header's name. Fields the chosen scheme does not use keep what was typed, so switching back finds it.

use orchester_core::{
  connection::mcp::{BasicUsername, McpAuthorization, McpAuthorizationError, McpHeaderName},
  id::SecretId,
  secret::Secret,
};

use super::McpServerErrors;
use crate::messages::Message;

/// What the Authorization select offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthorizationChoice {
  #[default]
  None,
  Bearer,
  /// A user name and a password, which an agent's start encodes.
  Basic,
  /// A Basic credential pasted whole.
  BasicCredential,
  /// A header of another name.
  Header,
}

/// The authorization fields as entered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthorizationText {
  pub choice: AuthorizationChoice,
  pub secret: Option<SecretId>,
  pub username: String,
  pub header: String,
}

impl AuthorizationText {
  pub fn of(authorization: &McpAuthorization) -> Self {
    let secret = authorization.secret();
    match authorization {
      McpAuthorization::None => Self::default(),
      McpAuthorization::Bearer { .. } => Self {
        choice: AuthorizationChoice::Bearer,
        secret,
        ..Self::default()
      },
      McpAuthorization::Basic { username, .. } => Self {
        choice: AuthorizationChoice::Basic,
        secret,
        username: username.as_str().to_owned(),
        ..Self::default()
      },
      McpAuthorization::BasicCredential { .. } => Self {
        choice: AuthorizationChoice::BasicCredential,
        secret,
        ..Self::default()
      },
      McpAuthorization::Header { name, .. } => Self {
        choice: AuthorizationChoice::Header,
        secret,
        header: name.as_str().to_owned(),
        ..Self::default()
      },
    }
  }

  /// The authorization these fields describe, its secret one of `secrets` (the project's active ones), or `None`
  /// with each field's reason in `errors`.
  pub(super) fn authorization(&self, errors: &mut McpServerErrors, secrets: &[Secret]) -> Option<McpAuthorization> {
    match self.choice {
      AuthorizationChoice::None => Some(McpAuthorization::None),
      AuthorizationChoice::Bearer => self
        .secret(errors, secrets)
        .map(|token| McpAuthorization::Bearer { token }),
      AuthorizationChoice::BasicCredential => self
        .secret(errors, secrets)
        .map(|credential| McpAuthorization::BasicCredential { credential }),
      AuthorizationChoice::Basic => {
        let username =
          BasicUsername::new(&self.username).map_err(|error| errors.username = Some(authorization_message(&error)));
        let password = self.secret(errors, secrets);
        Some(McpAuthorization::Basic {
          username: username.ok()?,
          password: password?,
        })
      }
      AuthorizationChoice::Header => {
        let name =
          McpHeaderName::new(&self.header).map_err(|error| errors.header = Some(authorization_message(&error)));
        let value = self.secret(errors, secrets);
        Some(McpAuthorization::Header {
          name: name.ok()?,
          value: value?,
        })
      }
    }
  }

  /// The secret chosen, if it is one of `secrets`; else its reason in `errors`.
  fn secret(&self, errors: &mut McpServerErrors, secrets: &[Secret]) -> Option<SecretId> {
    let error = match self.secret {
      Some(id) if secrets.iter().any(|secret| secret.id() == id) => return Some(id),
      Some(_) => "connections.mcp.errors.secretGone",
      None => "connections.mcp.errors.secretRequired",
    };
    errors.secret = Some(Message::new(error));
    None
  }
}

fn authorization_message(error: &McpAuthorizationError) -> Message {
  match error {
    McpAuthorizationError::BlankUsername => Message::new("connections.mcp.errors.usernameRequired"),
    McpAuthorizationError::UsernameColon => Message::new("connections.mcp.errors.usernameColon"),
    McpAuthorizationError::UsernameControl => Message::new("connections.mcp.errors.usernameControl"),
    McpAuthorizationError::UsernameTooLong => {
      Message::new("connections.mcp.errors.usernameTooLong").with("max", BasicUsername::MAX_CHARS.to_string())
    }
    McpAuthorizationError::BlankHeader => Message::new("connections.mcp.errors.headerRequired"),
    McpAuthorizationError::HeaderInvalid(header) => {
      Message::new("connections.mcp.errors.headerInvalid").with("header", header.clone())
    }
    McpAuthorizationError::HeaderTooLong => {
      Message::new("connections.mcp.errors.headerTooLong").with("max", McpHeaderName::MAX_CHARS.to_string())
    }
    McpAuthorizationError::HeaderReserved(header) => {
      Message::new("connections.mcp.errors.headerReserved").with("header", header.clone())
    }
  }
}
