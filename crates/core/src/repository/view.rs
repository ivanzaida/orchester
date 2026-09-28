//! A repository as the signed-in account sees it, read with `gh repo view <host/owner/name>
//! --json …` (read-only).

use std::path::Path;

use serde::Deserialize;

use super::{
  RepoPermission, RepositoryError,
  url::{GitHubRepo, redact_userinfo},
};
use crate::{
  command::{Command, CommandOutput},
  connection::gh_command,
  run::BranchName,
};

const GH: &str = "gh";
const VIEW_FIELDS: &str = "nameWithOwner,defaultBranchRef,viewerPermission";

/// What GitHub reports about a repository for the signed-in account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryView {
  /// The repository under its current name: GitHub follows renames and transfers.
  pub repo: GitHubRepo,
  pub default_branch: BranchName,
  pub permission: RepoPermission,
}

pub fn gh_repo_view_command(executable: &Path, repo: &GitHubRepo) -> Command {
  let args = [
    "repo".to_owned(),
    "view".to_owned(),
    repo.to_string(),
    "--json".to_owned(),
    VIEW_FIELDS.to_owned(),
  ];
  gh_command(executable, args.to_vec())
}

/// Reads `gh repo view` output for `requested`. A failed run is classified from `gh`'s error
/// message.
pub fn parse_gh_repo_view(requested: &GitHubRepo, output: &CommandOutput) -> Result<RepositoryView, RepositoryError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct View {
    name_with_owner: String,
    default_branch_ref: Option<BranchRef>,
    viewer_permission: Option<String>,
  }
  #[derive(Deserialize)]
  struct BranchRef {
    name: String,
  }
  if !output.success() {
    return Err(classify_failure(requested, &output.stderr));
  }
  let view: View = serde_json::from_str(output.stdout.trim()).map_err(|error| unrecognized(&error.to_string()))?;
  let repo = GitHubRepo::from_name_with_owner(requested.host().clone(), &view.name_with_owner)
    .map_err(|error| unrecognized(&error.to_string()))?;
  let permission = match view.viewer_permission.as_deref() {
    None | Some("" | "NONE") => return Err(RepositoryError::NoAccess(repo)),
    Some(value) => RepoPermission::parse(value).ok_or_else(|| unrecognized(value))?,
  };
  let default_branch = match view.default_branch_ref {
    Some(BranchRef { name }) if !name.is_empty() => {
      BranchName::new(&name).map_err(|error| unrecognized(&error.to_string()))?
    }
    _ => return Err(RepositoryError::NoDefaultBranch(repo)),
  };
  Ok(RepositoryView {
    repo,
    default_branch,
    permission,
  })
}

pub(crate) fn unrecognized(detail: &str) -> RepositoryError {
  RepositoryError::Unrecognized {
    program: GH,
    detail: redact_userinfo(detail.trim()),
  }
}

/// Maps `gh`'s error message to what went wrong, e.g. `GraphQL: Could not resolve to a
/// Repository with the name 'owner/name'. (repository)` or `HTTP 401: Bad credentials`.
pub(crate) fn classify_failure(requested: &GitHubRepo, stderr: &str) -> RepositoryError {
  const NOT_SIGNED_IN: [&str; 3] = ["gh auth login", "http 401", "bad credentials"];
  const NOT_FOUND: [&str; 1] = ["could not resolve to a repository"];
  const NO_ACCESS: [&str; 3] = ["http 403", "resource not accessible", "saml"];
  const NETWORK: [&str; 6] = [
    "error connecting to",
    "dial tcp",
    "no such host",
    "i/o timeout",
    "connection refused",
    "tls handshake",
  ];
  /// A 504 reads "We couldn't respond to your request in time"; a 502 answer without a body
  /// leaves `gh` with "unexpected end of JSON input".
  const SERVER: [&str; 5] = [
    "gateway",
    "server error",
    "service unavailable",
    "couldn't respond to your request in time",
    "unexpected end of json input",
  ];
  let message = stderr.to_ascii_lowercase();
  let mentions = |patterns: &[&str]| patterns.iter().any(|pattern| message.contains(pattern));
  let status = server_status(&message);
  if message.contains("rate limit") {
    RepositoryError::RateLimited
  } else if status.is_some() || mentions(&SERVER) {
    RepositoryError::ServerError {
      host: requested.host().clone(),
      status,
      detail: redact_userinfo(stderr.trim()),
    }
  } else if mentions(&NOT_SIGNED_IN) {
    RepositoryError::NotSignedIn(requested.host().clone())
  } else if mentions(&NOT_FOUND) {
    RepositoryError::NotFound(requested.clone())
  } else if mentions(&NO_ACCESS) {
    RepositoryError::NoAccess(requested.clone())
  } else if mentions(&NETWORK) {
    RepositoryError::Network {
      host: requested.host().clone(),
      detail: redact_userinfo(stderr.trim()),
    }
  } else {
    unrecognized(stderr)
  }
}

/// The HTTP 5xx status `gh` names in `message` (lower case), as in `HTTP 504: We couldn't
/// respond to your request in time`.
fn server_status(message: &str) -> Option<u16> {
  message.match_indices("http 5").find_map(|(at, _)| {
    let digits = message.get(at + "http ".len()..at + "http ".len() + 3)?;
    let status: u16 = digits.parse().ok()?;
    (500..600).contains(&status).then_some(status)
  })
}
