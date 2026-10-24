//! What one invocation sends an HTTP server ([`crate::connection::mcp::McpAuthorization`]) with its secret read:
//! a bearer token, which Codex takes alone (`bearer_token_env_var`), or a header's whole value, which both CLIs take
//! by the header's name. Basic credentials become `Basic <base64 of username:password>` (RFC 7617, UTF-8) here, in
//! memory that is wiped when it is dropped.

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
  },
}

impl HttpAuthorization {
  /// `authorization`, its secret's value given by `read`.
  pub fn read<E>(
    authorization: &McpAuthorization,
    read: impl FnOnce(SecretId) -> Result<Arc<SecretValue>, E>,
  ) -> Result<Self, E> {
    let header = |value| Self::Header {
      name: McpHeaderName::authorization(),
      value,
    };
    Ok(match authorization {
      McpAuthorization::None => Self::None,
      McpAuthorization::Bearer { token } => Self::Bearer(read(*token)?),
      McpAuthorization::Basic { username, password } => {
        let password = read(*password)?;
        let pair = Zeroizing::new(format!("{}:{}", username.as_str(), password.expose()));
        let mut value = Zeroizing::new(BASIC.to_owned());
        STANDARD.encode_string(pair.as_bytes(), &mut value);
        header(Arc::new(secret(value)))
      }
      McpAuthorization::BasicCredential { credential } => {
        let credential = read(*credential)?;
        let whole = credential
          .expose()
          .get(..BASIC.len())
          .is_some_and(|start| start.eq_ignore_ascii_case(BASIC));
        if whole {
          header(credential)
        } else {
          header(Arc::new(secret(Zeroizing::new(format!(
            "{BASIC}{}",
            credential.expose()
          )))))
        }
      }
      McpAuthorization::Header { name, value } => Self::Header {
        name: name.clone(),
        value: read(*value)?,
      },
    })
  }
}

/// `value` as a secret, its text moved, not copied.
fn secret(mut value: Zeroizing<String>) -> SecretValue {
  SecretValue::new(std::mem::take(&mut *value)).expect("a value that starts with the scheme's name is never blank")
}
