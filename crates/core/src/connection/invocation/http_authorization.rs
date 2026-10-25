//! What one invocation sends an HTTP server ([`crate::connection::mcp::McpAuthorization`]) with its secret read:
//! a bearer token, which Codex takes alone (`bearer_token_env_var`), or a header's whole value, which both CLIs take
//! by the header's name. Basic credentials become `Basic <base64 of username:password>` (RFC 7617, UTF-8) here, in
//! memory that is wiped when it is dropped. A server may repeat what it was sent in another form, so the project
//! secret and the bare credential are kept beside the header's value, for what the agent's activity must hide.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use zeroize::Zeroizing;

use crate::{
  connection::mcp::{McpAuthorization, McpHeaderName},
  id::SecretId,
  secret::SecretValue,
};

/// The Basic scheme as a header value starts with it.
const BASIC: &str = "Basic ";

/// What an HTTP server is sent with every request; `Debug` shows no value.
#[derive(Clone, Debug)]
pub enum HttpAuthorization {
  None,
  /// `Authorization: Bearer <token>`.
  Bearer(Arc<SecretValue>),
  /// `<name>: <value>`, the value whole.
  Header {
    name: McpHeaderName,
    value: Arc<SecretValue>,
    /// What the value is made of, hidden like it but never sent: a Basic password, the bare base64 credential and
    /// the credential as the project stores it.
    parts: Vec<Arc<SecretValue>>,
  },
}

impl HttpAuthorization {
  /// `authorization`, its secret's value given by `read`.
  pub fn read<E>(
    authorization: &McpAuthorization,
    read: impl FnOnce(SecretId) -> Result<Arc<SecretValue>, E>,
  ) -> Result<Self, E> {
    let header = |value, parts| Self::Header {
      name: McpHeaderName::authorization(),
      value,
      parts,
    };
    Ok(match authorization {
      McpAuthorization::None => Self::None,
      McpAuthorization::Bearer { token } => Self::Bearer(read(*token)?),
      McpAuthorization::Basic { username, password } => {
        let password = read(*password)?;
        let pair = Zeroizing::new(format!("{}:{}", username.as_str(), password.expose()));
        let mut encoded = Zeroizing::new(String::new());
        STANDARD.encode_string(pair.as_bytes(), &mut encoded);
        let value = Zeroizing::new(format!("{BASIC}{}", encoded.as_str()));
        let parts = [Some(password), part(encoded)].into_iter().flatten().collect();
        header(Arc::new(secret(value)), parts)
      }
      McpAuthorization::BasicCredential { credential } => {
        let credential = read(*credential)?;
        let bare = credential
          .expose()
          .get(..BASIC.len())
          .filter(|start| start.eq_ignore_ascii_case(BASIC))
          .map(|_| Zeroizing::new(credential.expose()[BASIC.len()..].to_owned()));
        match bare {
          Some(bare) => header(credential, part(bare).into_iter().collect()),
          None => {
            let value = Zeroizing::new(format!("{BASIC}{}", credential.expose()));
            header(Arc::new(secret(value)), vec![credential])
          }
        }
      }
      McpAuthorization::Header { name, value } => Self::Header {
        name: name.clone(),
        value: read(*value)?,
        parts: Vec::new(),
      },
    })
  }

  /// Every secret value this authorization holds, sent or not, for what must never show it.
  pub fn hidden(&self) -> Vec<&Arc<SecretValue>> {
    match self {
      Self::None => Vec::new(),
      Self::Bearer(token) => vec![token],
      Self::Header { value, parts, .. } => std::iter::once(value).chain(parts).collect(),
    }
  }
}

/// `value` as a secret, its text moved, not copied.
fn secret(mut value: Zeroizing<String>) -> SecretValue {
  SecretValue::new(std::mem::take(&mut *value)).expect("a value that starts with the scheme's name is never blank")
}

/// `value` as a secret to hide; none when it is blank, which hides nothing.
fn part(mut value: Zeroizing<String>) -> Option<Arc<SecretValue>> {
  SecretValue::new(std::mem::take(&mut *value)).ok().map(Arc::new)
}
