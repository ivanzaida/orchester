//! What the step reads of its project: its name and folder, how it stands with its server, the repository the server
//! names for it (the round keeps it in the project's cache before the tasks) and the checkout's `origin`, read once.

use std::path::PathBuf;

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx};
use orchester_core::{id::ProjectId, repository::RepositoryIdentity};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  sharing::{
    SharingError,
    project::{link::checkout_origin, state::ProjectSharing},
  },
};

use crate::{
  screens::settings::orchester::{Said, sharing_said},
  services::Services,
};

/// How the project stands as the step reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Standing {
  /// No project with its id is listed any more.
  Missing,
  Local {
    name: String,
  },
  Linked(Box<Linked>),
}

impl DevtoolsInspectable for Standing {}

/// A project linked to a server's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Linked {
  pub name: String,
  pub root: PathBuf,
  pub sharing: ProjectSharing,
  /// The repository the server names for it, once a round read that: `None` until then.
  pub hint: Option<Option<RepositoryIdentity>>,
}

/// The checkout's `origin`, `None` without a network one, once read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Origin(pub Option<RepositoryIdentity>);

impl DevtoolsInspectable for Origin {}

/// The project as read at `revision`; `None` until the first reading ended. The previous reading stays while it is read
/// again.
pub(super) fn standing(ctx: &mut Ctx, revision: u64, project: ProjectId) -> Result<Option<Standing>, Said> {
  let services = Services::of(ctx);
  let state = ctx
    .future(
      (revision, project, services.workspace, services.servers),
      |(_, project, workspace, servers)| async move {
        background::run(move || -> Result<Standing, SharingError> {
          let listed = workspace.projects()?.into_iter().find(|listed| listed.id == project);
          let Some(listed) = listed else {
            return Ok(Standing::Missing);
          };
          let name = listed.name.as_str().to_owned();
          let Some(sharing) = servers.project_sharing(&listed.repo_root)? else {
            return Ok(Standing::Local { name });
          };
          let hint = ProjectDb::open(&listed.repo_root)?
            .remote_hint()?
            .map(|(hint, _)| hint.and_then(|hint| RepositoryIdentity::parse(&hint).ok()));
          Ok(Standing::Linked(Box::new(Linked {
            name,
            root: listed.repo_root,
            sharing,
            hint,
          })))
        })
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map_err(|error| {
          tracing::warn!(%error, %project, "the first download's project could not be read");
          sharing_said(&error)
        })
      },
    )
    .state()
    .get();
  match (state.data, state.error) {
    (Some(standing), _) => Ok(Some(standing)),
    (None, Some(said)) => Err(said),
    (None, None) => Ok(None),
  }
}

/// The `origin` of the checkout at `root`, read once for it; `None` until it is, and without a checkout.
pub(super) fn origin(ctx: &mut Ctx, root: Option<PathBuf>) -> Option<Origin> {
  let programs = Services::of(ctx).programs;
  ctx
    .future((root, programs), |(root, programs)| async move {
      let Some(root) = root else {
        return Ok(None);
      };
      let origin = background::run(move || checkout_origin(&programs, &root))
        .await
        .map_err(|_| ())?;
      Ok::<_, ()>(Some(Origin(origin)))
    })
    .state()
    .get()
    .data
    .flatten()
}
