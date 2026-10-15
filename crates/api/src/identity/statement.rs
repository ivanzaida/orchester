use serde::de::DeserializeOwned;

use super::{IdentityError, PublicKey, jws};

/// The payload of a statement a team server signed, **not verified**: for reading which server signed it (to find
/// its key) or, for a link statement, the key it carries. Act on it only after [`verify_statement`].
pub fn read_statement<T: DeserializeOwned>(compact: &str, typ: &str) -> Result<T, IdentityError> {
  jws::parse(&jws::decode(compact, typ)?.payload)
}

/// The payload of a statement of type `typ` that `key` signed, its header naming that key.
pub fn verify_statement<T: DeserializeOwned>(compact: &str, typ: &str, key: &PublicKey) -> Result<T, IdentityError> {
  let compact = jws::decode(compact, typ)?;
  if compact.header.kid.as_deref() != Some(key.kid().as_str()) {
    return Err(IdentityError::UnknownKey);
  }
  key.verify(&compact.signing_input, &compact.signature)?;
  jws::parse(&compact.payload)
}
