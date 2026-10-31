//! The CI run that uploaded a release (`docs/cloud-api.md`, "Uploading a release"): CI names it when it creates the
//! release, and the cloud only stores it and shows it in the admin.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::ReleaseFormatError;

/// The longest CI run link the contract takes.
const URL_MAX: usize = 2048;

/// The CI run that uploaded a release: its number as the CI shows it (`#48`) and a link to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiRun {
  pub number: u64,
  pub url: CiRunUrl,
}

/// A link to a CI run: `https://`, at most 2048 characters of printable ASCII without spaces. The cloud does not
/// follow it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CiRunUrl(String);

impl CiRunUrl {
  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    let printable = text.bytes().all(|byte| byte.is_ascii_graphic());
    let https = matches!(text.strip_prefix("https://"), Some(rest) if !rest.is_empty());
    if !https || text.len() > URL_MAX || !printable {
      return Err(ReleaseFormatError::CiRunUrl);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for CiRunUrl {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for CiRunUrl {
  type Error = ReleaseFormatError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::parse(&text)
  }
}

impl From<CiRunUrl> for String {
  fn from(url: CiRunUrl) -> Self {
    url.0
  }
}
