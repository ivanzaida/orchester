//! `GET /admin/projects`: the projects the server admin's caller manages.

use axum::{Json, extract::State};
use orchester_api::{ManagedProjectResponse, ManagedProjects};
use orchester_core::id::ProjectId;

use super::extract::{Auth, Base, Id};
use crate::{Server, db, error::ServerError, sharing::managed};

pub(super) async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  Base(base): Base,
) -> Result<Json<ManagedProjects>, ServerError> {
  let projects = server
    .snapshot(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      managed::list(db, &current.user, &base, now)
    })
    .await?;
  Ok(Json(ManagedProjects { projects }))
}

pub(super) async fn show(
  State(server): State<Server>,
  Auth(auth): Auth,
  Base(base): Base,
  Id(project): Id<ProjectId>,
) -> Result<Json<ManagedProjectResponse>, ServerError> {
  let project = server
    .snapshot(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      managed::show(db, &current.user, project, &base, now)
    })
    .await?;
  Ok(Json(ManagedProjectResponse { project }))
}
