//! Whether a server has a project for a repository (`docs/architecture/server.md`, "Finding the
//! project"): asked of the servers the person signed in to, never of an address a repository names,
//! with the repository's identity only.

use orchester_api::{ErrorCode, SharingErrorDetails};
use orchester_core::repository::RepositoryIdentity;

use crate::sharing::{ServerEntry, Servers, SharingError};

/// What a server has for a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
  NotShared,
  /// Shared there as the project with `key`, and the person is a member.
  Shared {
    key: String,
  },
  /// Shared there, and the person is not a member; `owners` can add them.
  NotMember {
    owners: Vec<String>,
  },
}

impl Servers {
  /// Asks `entry` whether it has a project for `identity`.
  pub async fn lookup(&self, entry: &ServerEntry, identity: RepositoryIdentity) -> Result<Lookup, SharingError> {
    let answer = self
      .with_token(entry, |client, token| async move {
        client.projects(&token, Some(&identity)).await
      })
      .await;
    match answer {
      Ok(projects) => Ok(
        projects
          .into_iter()
          .next()
          .map_or(Lookup::NotShared, |project| Lookup::Shared { key: project.key }),
      ),
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotMember) => {
        let owners = match error.details() {
          Some(SharingErrorDetails::Owners { owners }) => {
            owners.iter().map(|owner| owner.display_name.clone()).collect()
          }
          _ => Vec::new(),
        };
        Ok(Lookup::NotMember { owners })
      }
      Err(error) => Err(error),
    }
  }
}
