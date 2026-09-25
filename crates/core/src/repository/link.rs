use super::{RemoteName, RepositoryError, RepositoryLink, remote::Remote};
use crate::{
  connection::{ConnectionStatus, GitHubConnection},
  github::GitHub,
  time::Timestamp,
};

/// Links the project to the GitHub repository its remote `remote` points at, through
/// `connection`: the remote must name a repository on the connection's host, `gh` must be
/// signed in there, and the account must see the repository. The account becomes the link's
/// expected login.
pub fn link_repository(
  github: &impl GitHub,
  connection: &GitHubConnection,
  remotes: &[Remote],
  remote: &RemoteName,
  now: Timestamp,
) -> Result<RepositoryLink, RepositoryError> {
  let found = remotes
    .iter()
    .find(|candidate| candidate.name() == remote)
    .ok_or_else(|| RepositoryError::NoGitHubRemote(remote.clone()))?;
  let repo = found
    .github_repo()
    .filter(|repo| repo.host() == connection.host())
    .ok_or_else(|| RepositoryError::NonGitHubRemote {
      remote: remote.clone(),
      url: found.url().to_owned(),
      host: connection.host().clone(),
    })?;
  let check = github.check(now);
  let Some(account) = check.result.signed_in_account() else {
    return Err(match check.result.status() {
      ConnectionStatus::SignInRequired => RepositoryError::NotSignedIn(connection.host().clone()),
      status => RepositoryError::GitHubUnavailable(status),
    });
  };
  let view = github.repository(&repo)?;
  Ok(RepositoryLink::new(
    connection.id(),
    remote.clone(),
    view,
    account.clone(),
    now,
  ))
}
