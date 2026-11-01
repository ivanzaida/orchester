//! A release's version and the names around it: the platforms and architectures it ships for, and its files' names
//! (`docs/cloud-api.md`, "App releases"). Its channels are core's ([`Channel`]), which the app's settings keep too.

use std::{cmp::Ordering, fmt, str::FromStr};

pub use orchester_core::app_update::Channel;
use serde::{Deserialize, Serialize};

use super::ReleaseFormatError;

/// The longest version the contract takes.
const VERSION_MAX: usize = 64;
/// The longest file name the contract takes.
const FILE_NAME_MAX: usize = 128;

/// An app version, `MAJOR.MINOR.PATCH` with an optional pre-release suffix `-beta.1`, as Semantic Versioning 2.0.0
/// orders them. Numbers have no leading zeros and fit in 32 bits; build metadata (`+…`) is refused, so two spellings
/// never name one version. At most 64 characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReleaseVersion {
  text: String,
  core: [u32; 3],
  pre: Vec<PreIdentifier>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PreIdentifier {
  Number(u64),
  Text(String),
}

impl ReleaseVersion {
  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    if text.is_empty() || text.len() > VERSION_MAX {
      return Err(ReleaseFormatError::Version);
    }
    let (core, pre) = match text.split_once('-') {
      Some((core, pre)) => (core, Some(pre)),
      None => (text, None),
    };
    let numbers: Vec<u32> = core
      .split('.')
      .map(|part| number(part).and_then(|value| u32::try_from(value).ok()))
      .collect::<Option<_>>()
      .ok_or(ReleaseFormatError::Version)?;
    let core: [u32; 3] = numbers.try_into().map_err(|_| ReleaseFormatError::Version)?;
    let pre = match pre {
      Some(pre) => pre.split('.').map(pre_identifier).collect::<Option<_>>(),
      None => Some(Vec::new()),
    }
    .ok_or(ReleaseFormatError::Version)?;
    Ok(Self {
      text: text.to_owned(),
      core,
      pre,
    })
  }

  pub fn as_str(&self) -> &str {
    &self.text
  }

  /// Whether the version has a pre-release suffix: such a release is offered on Beta only.
  pub fn is_pre_release(&self) -> bool {
    !self.pre.is_empty()
  }

  /// Whether a release of this version may be offered on `channel`.
  pub fn allowed_on(&self, channel: Channel) -> bool {
    channel == Channel::Beta || !self.is_pre_release()
  }
}

/// A decimal number without a leading zero.
fn number(text: &str) -> Option<u64> {
  let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
  if !digits || (text.len() > 1 && text.starts_with('0')) {
    return None;
  }
  text.parse().ok()
}

fn pre_identifier(text: &str) -> Option<PreIdentifier> {
  if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') {
    return None;
  }
  if text.bytes().all(|byte| byte.is_ascii_digit()) {
    return number(text).map(PreIdentifier::Number);
  }
  Some(PreIdentifier::Text(text.to_owned()))
}

impl Ord for ReleaseVersion {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .core
      .cmp(&other.core)
      .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
        (true, true) => Ordering::Equal,
        // A pre-release comes before the release it leads to.
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => self.pre.cmp(&other.pre),
      })
  }
}

impl PartialOrd for ReleaseVersion {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PreIdentifier {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (Self::Number(left), Self::Number(right)) => left.cmp(right),
      (Self::Number(_), Self::Text(_)) => Ordering::Less,
      (Self::Text(_), Self::Number(_)) => Ordering::Greater,
      (Self::Text(left), Self::Text(right)) => left.cmp(right),
    }
  }
}

impl PartialOrd for PreIdentifier {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl fmt::Display for ReleaseVersion {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.text)
  }
}

impl FromStr for ReleaseVersion {
  type Err = ReleaseFormatError;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    Self::parse(text)
  }
}

impl TryFrom<String> for ReleaseVersion {
  type Error = ReleaseFormatError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::parse(&text)
  }
}

impl From<ReleaseVersion> for String {
  fn from(version: ReleaseVersion) -> Self {
    version.text
  }
}

/// An operating system a release ships files for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePlatform {
  Windows,
  Macos,
}

impl ReleasePlatform {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Windows => "windows",
      Self::Macos => "macos",
    }
  }

  pub fn parse(text: &str) -> Option<Self> {
    [Self::Windows, Self::Macos]
      .into_iter()
      .find(|platform| platform.as_str() == text)
  }
}

/// A processor architecture a release ships files for, named as Rust's `std::env::consts::ARCH` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReleaseArch {
  #[serde(rename = "x86_64")]
  X86_64,
  #[serde(rename = "aarch64")]
  Aarch64,
}

impl ReleaseArch {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::X86_64 => "x86_64",
      Self::Aarch64 => "aarch64",
    }
  }

  pub fn parse(text: &str) -> Option<Self> {
    [Self::X86_64, Self::Aarch64]
      .into_iter()
      .find(|arch| arch.as_str() == text)
  }
}

/// A release file's name, e.g. `Orchester-0.4.0-windows-x86_64.msi`: 1–128 characters of ASCII letters, digits,
/// `.`, `_`, `+` and `-`, not starting with `.` or `-`. It goes into URL paths and the signed message as it is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReleaseFileName(String);

impl ReleaseFileName {
  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    let allowed = text
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'));
    let leading = text.starts_with('.') || text.starts_with('-');
    if text.is_empty() || text.len() > FILE_NAME_MAX || !allowed || leading {
      return Err(ReleaseFormatError::FileName);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for ReleaseFileName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for ReleaseFileName {
  type Error = ReleaseFormatError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::parse(&text)
  }
}

impl From<ReleaseFileName> for String {
  fn from(name: ReleaseFileName) -> Self {
    name.0
  }
}
