//! Finding a server project (`docs/architecture/server.md`, rows 49–50, "Finding the project"): by its address, the
//! slug on a server the person joined, or among the person's projects there by the repository a project was set up
//! for, which is only a hint. Never asked of an address a repository names before the person confirms it.

use orchester_api::{ErrorCode, SharedProject, SharingErrorDetails};
use orchester_core::repository::RepositoryIdentity;

use crate::sharing::{ServerEntry, Servers, SharingError};

/// What a server has for a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
  NotShared,
  /// A project of the person's there was set up for the repository; it has `key`.
  Shared {
    key: String,
  },
  /// Shared there, and the person is not a member; `owners` can add them. A lookup by repository cannot tell this:
  /// only a lookup by address can ([`Servers::find_project`]).
  NotMember {
    owners: Vec<String>,
  },
}

impl Servers {
  /// Whether one of the person's projects on `entry` was set up for `identity` (its remote hint).
  pub async fn lookup(&self, entry: &ServerEntry, identity: RepositoryIdentity) -> Result<Lookup, SharingError> {
    Ok(
      self
        .project_by_hint(entry, &identity)
        .await?
        .map_or(Lookup::NotShared, |project| Lookup::Shared { key: project.key }),
    )
  }

  /// The project at `slug` on `entry`. Not a member: [`SharingError::NoProject`] with its name and owners; no
  /// project there: the same without a name.
  pub async fn find_project(&self, entry: &ServerEntry, slug: &str) -> Result<SharedProject, SharingError> {
    let slug = slug.to_owned();
    let answer = self
      .with_token(entry, |client, token| async move {
        client.project_by_slug(&token, &slug).await
      })
      .await;
    match answer {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotMember) => {
        let (name, owners) = match error.details() {
          Some(SharingErrorDetails::Project { name, owners }) => (
            Some(name.clone()),
            owners.iter().map(|owner| owner.display_name.clone()).collect(),
          ),
          _ => (None, Vec::new()),
        };
        Err(SharingError::NoProject { name, owners })
      }
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotFound) => Err(SharingError::NoProject {
        name: None,
        owners: Vec::new(),
      }),
      answer => answer,
    }
  }

  /// The person's project on `entry` that was set up for `identity`, if there is one.
  pub(super) async fn project_by_hint(
    &self,
    entry: &ServerEntry,
    identity: &RepositoryIdentity,
  ) -> Result<Option<SharedProject>, SharingError> {
    let projects = self
      .with_token(entry, |client, token| async move { client.projects(&token).await })
      .await?;
    Ok(
      projects
        .into_iter()
        .find(|project| project.remote_hint.as_ref() == Some(identity)),
    )
  }
}
