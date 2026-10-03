use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use zeroize::Zeroize;

/// A credential at the JSON boundary. Serialization is intentional; diagnostics are redacted.
pub struct Credential(String);

impl Credential {
  pub fn new(value: String) -> Self {
    // Validation belongs to the endpoint: an empty current password is invalid credentials,
    // an empty new password is invalid input, and all spaces can be a valid password.
    Self(value)
  }

  pub fn expose(&self) -> &str {
    &self.0
  }
}

impl Drop for Credential {
  fn drop(&mut self) {
    self.0.zeroize();
  }
}

impl fmt::Debug for Credential {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("Credential(<redacted>)")
  }
}

impl Serialize for Credential {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.expose())
  }
}

impl<'de> Deserialize<'de> for Credential {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    // Never forward a deserializer error that might include the rejected credential.
    let value = String::deserialize(deserializer).map_err(|_| D::Error::custom("invalid credential"))?;
    Ok(Self::new(value))
  }
}
