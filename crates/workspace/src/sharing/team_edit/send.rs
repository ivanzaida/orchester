//! Sending an admin's change to the team server and keeping its answer in memory: a new version, published on the
//! latest version read this session (`POST …/resources/{resource}/versions`), and a new resource with its first version
//! (`POST …/resources`). A skill's parts the server does not have are uploaded from the project's blobs by their hash,
//! then the change is sent again. Nothing is written to the project's database.

use std::{path::Path, sync::Arc};

use orchester_api::{ErrorCode, ResourceCreate, SharedVersion, SharingErrorDetails, VersionPublish};
use orchester_core::{
  blob::ContentHash,
  id::{IdempotencyKey, ResourceId},
  resource::{ResourceKind, Slug, VersionNumber},
  store::BlobStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{EditRefused, NewTeamResource, Publication, Refusal, recorded};
use crate::{
  background,
  messages::{self, Message},
  sharing::{Servers, SharingError, project::ProjectServer, sync::ConfigServer, team::said},
  team_config::{TeamConfig, TeamRead, TeamResource},
};

/// The slug of a resource whose name has no ASCII letters or digits.
const FALLBACK_SLUG: &str = "resource";

impl Servers {
  /// Publishes `publication` as the next version of the team resource `resource` of the project at `repo_root`, made
  /// on version `base` (`None` before its first); the number the server gave it, which memory holds from now on. A
  /// `base` older than the latest version this session read of it is refused here as [`EditRefused::Changed`], and
  /// nothing is sent: the change would undo what was published since.
  pub async fn publish_team(
    &self,
    repo_root: &Path,
    resource: ResourceId,
    (base, publication): (Option<VersionNumber>, Publication),
  ) -> Result<VersionNumber, EditRefused> {
    let config = self.writable_config(repo_root)?;
    let latest = config
      .resource(resource)
      .ok_or_else(|| store_failure(&StoreError::ResourceNotFound(resource)))?
      .number();
    if let Some(latest) = latest.filter(|latest| Some(*latest) != base) {
      return Err(EditRefused::Changed { latest });
    }
    let server = self.config_writer(repo_root).await?;
    let publish = VersionPublish {
      idempotency_key: IdempotencyKey::generate(),
      based_on: base,
      body: publication.body,
      references: publication.references,
    };
    let sent = delivered(&server, repo_root, || server.publish_version(resource, publish.clone())).await;
    match sent {
      Ok(version) => {
        let number = version.number;
        self.keep_published(repo_root, version);
        Ok(number)
      }
      Err(error) => Err(self.refusal(repo_root, &server, &error)),
    }
  }

  /// Creates `new` as a team resource of the project at `repo_root` with its first version; its id, which memory holds
  /// from now on.
  pub async fn create_team(&self, repo_root: &Path, new: NewTeamResource) -> Result<ResourceId, EditRefused> {
    let config = self.writable_config(repo_root)?;
    let kind = orchester_api::SharedResourceKind::from_core(new.kind)
      .ok_or_else(|| store_failure(&StoreError::SharedProject))?;
    let slug = slug_for(&config, new.kind, &new.name);
    let server = self.config_writer(repo_root).await?;
    let create = ResourceCreate {
      idempotency_key: IdempotencyKey::generate(),
      id: new.id,
      kind,
      slug: slug.as_str().to_owned(),
      name: new.name.as_str().to_owned(),
      version: orchester_api::VersionContent {
        body: new.publication.body,
        references: new.publication.references,
      },
    };
    match delivered(&server, repo_root, || server.create_resource(create.clone())).await {
      Ok(created) => {
        let id = created.resource.id;
        if let Some(kept) = TeamResource::of(&created.resource, Some(created.version.into())) {
          self.inner.workspace.team.update(repo_root, |config| {
            config.keep(kept);
            true
          });
        }
        self.shared_changed();
        Ok(id)
      }
      Err(error) => Err(self.refusal(repo_root, &server, &error)),
    }
  }

  /// The configuration read this session of the project at `root`, while its team server can be reached: what a change
  /// is made on.
  fn writable_config(&self, root: &Path) -> Result<Arc<TeamConfig>, EditRefused> {
    match self.inner.workspace.team.read(root) {
      TeamRead::Reading => Err(EditRefused::Failed(Message::new("teamConfig.reading"))),
      TeamRead::Unreachable(unreached)
      | TeamRead::Read {
        unreached: Some(unreached),
        ..
      } => Err(EditRefused::Unreachable(unreached.reason)),
      TeamRead::Read {
        config,
        unreached: None,
      } => Ok(config),
    }
  }

  /// The team server of the project at `root`, which takes a change of the team's configuration from the person: they
  /// are an admin as this computer last read the members, and the server can be reached.
  async fn config_writer(&self, root: &Path) -> Result<ProjectServer, EditRefused> {
    let (servers, root) = (self.clone(), root.to_owned());
    background::run(move || {
      let server = ProjectServer::open(&servers, &root)
        .map_err(|error| EditRefused::Failed(said(&error, None)))?
        .ok_or_else(|| store_failure(&StoreError::NotShared))?;
      let admin = ProjectDb::open(&root)
        .and_then(|db| db.may_change_configuration())
        .map_err(|error| store_failure(&error))?;
      if !admin {
        return Err(EditRefused::Refused(Refusal::NotAdminHere));
      }
      server
        .writable()
        .map_err(|error| EditRefused::Unreachable(said(&error, Some(&server.entry().address))))?;
      Ok(server)
    })
    .await
    .map_err(EditRefused::Failed)?
  }

  /// Keeps `version`, which the server numbered, as the latest version of its resource in memory.
  fn keep_published(&self, root: &Path, version: SharedVersion) {
    let resource = version.resource;
    self
      .inner
      .workspace
      .team
      .update(root, |config| config.published(resource, version.into()));
    self.shared_changed();
  }

  /// Why the server did not take a change. A version someone published first is kept in memory as the resource's
  /// latest, which the edit starts from again.
  fn refusal(&self, root: &Path, server: &ProjectServer, error: &SharingError) -> EditRefused {
    if let SharingError::Client(client) = error {
      match (client.code(), client.details()) {
        (Some(ErrorCode::VersionConflict), Some(SharingErrorDetails::Version { latest })) => {
          let number = latest.number;
          self.keep_published(root, (**latest).clone());
          return EditRefused::Changed { latest: number };
        }
        (Some(code), details) => return EditRefused::Refused(Refusal::of_code(&recorded(code, details))),
        (None, _) => {}
      }
    }
    let words = said(error, Some(&server.entry().address));
    if error.unreached() {
      EditRefused::Unreachable(words)
    } else {
      EditRefused::Failed(words)
    }
  }
}

/// What `attempt` answers, after uploading from the project's blobs the skill parts the server says it lacks.
async fn delivered<T, F, Fut>(server: &ProjectServer, root: &Path, attempt: F) -> Result<T, SharingError>
where
  F: Fn() -> Fut,
  Fut: Future<Output = Result<T, SharingError>>,
{
  match attempt().await {
    Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::MissingFiles) => {
      let Some(SharingErrorDetails::Files { hashes }) = error.details() else {
        return Err(SharingError::InvalidAnswer("missing files"));
      };
      for hash in hashes.clone() {
        upload(server, root, hash).await?;
      }
      attempt().await
    }
    answer => answer,
  }
}

async fn upload(server: &ProjectServer, root: &Path, hash: ContentHash) -> Result<(), SharingError> {
  let (root, read) = (root.to_owned(), hash.clone());
  let content = background::run(move || ProjectDb::open(&root).and_then(|db| db.blob(&read)))
    .await
    .map_err(|_| SharingError::Interrupted)??;
  server.upload_file(hash, content).await?;
  Ok(())
}

/// A slug for a new resource of `kind` named `name`, none of the team's resources of that kind has by its name. The
/// server refuses one it has all the same (`409 slug_taken`), which says the name is taken.
fn slug_for(config: &TeamConfig, kind: ResourceKind, name: &orchester_core::name::Name) -> Slug {
  let taken: Vec<Slug> = config
    .resources(kind)
    .into_iter()
    .chain(config.archived_of(kind))
    .filter_map(|resource| Slug::from_name(resource.name.as_str()))
    .collect();
  let fallback = Slug::new(FALLBACK_SLUG).expect("the fallback slug is valid");
  Slug::unique_for(name, &fallback, &taken)
}

fn store_failure(error: &StoreError) -> EditRefused {
  EditRefused::Failed(messages::store_error(error))
}
