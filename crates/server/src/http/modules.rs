//! A shared project's modules (`docs/architecture/server.md`, "Modules"): every member reads them with the moves of
//! tasks into and out of them, makes them and changes them.

use axum::{
  Json, Router,
  extract::{Query, State},
  http::StatusCode,
  routing::get,
};
use orchester_api::{ErrorCode, ModuleCreate, ModulePatch, ModuleResponse, Modules};
use orchester_core::id::{ModuleId, ProjectId};
use serde::Deserialize;

use super::extract::{Auth, Id, SafeJson};
use crate::{
  Server,
  error::ServerError,
  sharing::{access, modules},
};

pub fn routes() -> Router<Server> {
  Router::new()
    .route("/projects/{project}/modules", get(list).post(create))
    .route("/projects/{project}/modules/{module}", axum::routing::patch(patch))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct After {
  /// The last move the caller has; every move without it.
  #[serde(default)]
  after: u64,
}

async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  query: Result<Query<After>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<Modules>, ServerError> {
  let Query(After { after }) = query.map_err(|_| ErrorCode::BadRequest)?;
  let response = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      Ok(Modules {
        modules: modules::list(db, project)?,
        changes: modules::changes(db, project, after)?,
      })
    })
    .await?;
  Ok(Json(response))
}

async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<ModuleCreate>,
) -> Result<(StatusCode, Json<ModuleResponse>), ServerError> {
  let module = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      modules::create(db, project, current.user.id, &input, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(ModuleResponse { module })))
}

async fn patch(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, module)): Id<(ProjectId, ModuleId)>,
  SafeJson(input): SafeJson<ModulePatch>,
) -> Result<Json<ModuleResponse>, ServerError> {
  let module = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      modules::patch(db, project, module, current.user.id, &input, now)
    })
    .await?;
  Ok(Json(ModuleResponse { module }))
}
