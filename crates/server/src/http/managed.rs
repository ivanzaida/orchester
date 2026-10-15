//! `GET /admin/projects`: the projects the server admin's caller manages.

use axum::{Json, extract::State, http::HeaderMap};
use orchester_api::{ManagedProjectResponse, ManagedProjects};
use orchester_core::id::ProjectId;

use super::{
  base_address,
  extract::{Auth, Id},
};
use crate::{Server, db, error::ServerError, sharing::managed};

pub(super) async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  headers: HeaderMap,
) -> Result<Json<ManagedProjects>, ServerError> {
  let base = base_address(&server, &headers);
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
  headers: HeaderMap,
  Id(project): Id<ProjectId>,
) -> Result<Json<ManagedProjectResponse>, ServerError> {
  let base = base_address(&server, &headers);
  let project = server
    .snapshot(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      managed::show(db, &current.user, project, &base, now)
    })
    .await?;
  Ok(Json(ManagedProjectResponse { project }))
}
