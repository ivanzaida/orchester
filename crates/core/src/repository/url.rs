//! GitHub repositories named by git remote URLs, and removing credentials from URLs.
//!
//! A remote URL may carry credentials in its user part (`https://user:token@host/…`). They are
//! dropped as soon as a URL is read, and redacted in any text that becomes an error.

use std::fmt;

use thiserror::Error;

use crate::connection::{GitHubHost, GitHubNameError};

/// Schemes git accepts for remotes on a server, as opposed to local paths.
const NETWORK_SCHEMES: [&str; 6] = ["https", "http", "ssh", "git+ssh", "ssh+git", "git"];
const GIT_SUFFIX: &str = ".git";
const REDACTED: &str = "***";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepoNameError {
  #[error(transparent)]
  Host(#[from] GitHubNameError),
  #[error("`{0}` is not a valid GitHub owner")]
  Owner(String),
  #[error("`{0}` is not a valid GitHub repository name")]
  Name(String),
}

/// A repository on a GitHub host. Displays as `host/owner/name`, the form `gh` accepts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GitHubRepo {
  host: GitHubHost,
  owner: String,
  name: String,
}

impl GitHubRepo {
  const MAX_OWNER_CHARS: usize = 39;
  const MAX_NAME_CHARS: usize = 100;

  pub fn new(host: GitHubHost, owner: &str, name: &str) -> Result<Self, RepoNameError> {
    let owner_valid = !owner.is_empty()
      && owner.len() <= Self::MAX_OWNER_CHARS
      && !owner.starts_with('-')
      && owner
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if !owner_valid {
      return Err(RepoNameError::Owner(owner.to_owned()));
    }
    let name_valid = !name.is_empty()
      && name.len() <= Self::MAX_NAME_CHARS
      && name != "."
      && name != ".."
      && !name.starts_with('-')
      && name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !name_valid {
      return Err(RepoNameError::Name(name.to_owned()));
    }
    Ok(Self {
      host,
      owner: owner.to_owned(),
      name: name.to_owned(),
    })
  }

  /// `owner/name` on `host`, as GitHub reports a repository's `nameWithOwner`.
  pub fn from_name_with_owner(host: GitHubHost, name_with_owner: &str) -> Result<Self, RepoNameError> {
    let (owner, name) = name_with_owner
      .split_once('/')
      .ok_or_else(|| RepoNameError::Name(name_with_owner.to_owned()))?;
    Self::new(host, owner, name)
  }

  /// The repository a remote URL points at: `https://host/owner/name(.git)`, `ssh://…` with the
  /// same path, or scp-like `user@host:owner/name(.git)`. Credentials and ports are ignored.
  /// `None` for anything else, including local paths and URLs with a longer path.
  pub fn from_remote_url(url: &str) -> Option<Self> {
    let (authority, path) = split_remote(url.trim())?;
    let host = host_of(authority);
    let path = path.trim_matches('/');
    let path = path.strip_suffix(GIT_SUFFIX).unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    Self::new(GitHubHost::new(host).ok()?, owner, name).ok()
  }

  pub const fn host(&self) -> &GitHubHost {
    &self.host
  }

  pub fn owner(&self) -> &str {
    &self.owner
  }

  pub fn name(&self) -> &str {
    &self.name
  }
}

impl fmt::Display for GitHubRepo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}/{}/{}", self.host, self.owner, self.name)
  }
}

/// `(authority, path)` of a network remote URL or an scp-like address.
pub(super) fn split_remote(url: &str) -> Option<(&str, &str)> {
  if let Some((scheme, rest)) = url.split_once("://") {
    if !NETWORK_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
      return None;
    }
    return Some(rest.split_once('/').unwrap_or((rest, "")));
  }
  // scp-like syntax: the part before the first colon is the host. As for git, a slash before
  // the colon (`./a:b`) or a single letter (`C:/repo`) means a local path.
  let (authority, path) = if let Some(end) = url.find("]:") {
    let (authority, path) = url.split_at(end + 1);
    (authority, path.strip_prefix(':')?)
  } else {
    url.split_once(':')?
  };
  (authority.len() > 1 && !authority.contains(['/', '\\'])).then_some((authority, path))
}

/// The host of an authority, without user part or port.
fn host_of(authority: &str) -> &str {
  let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
  host.split_once(':').map_or(host, |(host, _)| host)
}

/// `url` without its user part, which may hold credentials.
pub fn strip_userinfo(url: &str) -> String {
  let url = url.trim();
  if let Some((scheme, rest)) = url.split_once("://") {
    let (authority, path) = rest.split_once('/').map_or((rest, None), |(a, p)| (a, Some(p)));
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    return match path {
      Some(path) => format!("{scheme}://{host}/{path}"),
      None => format!("{scheme}://{host}"),
    };
  }
  match url.split_once(':') {
    Some((authority, path)) if authority.len() > 1 && !authority.contains(['/', '\\']) => {
      let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
      format!("{host}:{path}")
    }
    _ => url.to_owned(),
  }
}

/// `text` with the user part of every `scheme://user@host` URL in it replaced, for program
/// output that is shown or kept.
pub fn redact_userinfo(text: &str) -> String {
  let mut redacted = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find("://") {
    let (before, after) = rest.split_at(start + "://".len());
    redacted.push_str(before);
    let end = after
      .find(|c: char| c == '/' || c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '(' | ')'))
      .unwrap_or(after.len());
    let authority = &after[..end];
    match authority.rfind('@') {
      Some(at) => {
        redacted.push_str(REDACTED);
        redacted.push_str(&authority[at..]);
      }
      None => redacted.push_str(authority),
    }
    rest = &after[end..];
  }
  redacted.push_str(rest);
  redacted
}
