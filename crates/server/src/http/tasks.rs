use axum::{
  Json, Router,
  extract::State,
  http::StatusCode,
  routing::{get, post},
};
use orchester_api::*;
use orchester_core::id::{ProjectId, TaskId};

use super::extract::{Auth, Id, SafeJson};
use crate::{
  Server,
  error::ServerError,
  sharing::{access, batches, events, organization, task_rows, task_write},
};

pub fn routes() -> Router<Server> {
  Router::new()
    .route("/projects/{project}/tasks", get(list).post(create))
    .route("/projects/{project}/tasks/import", post(import))
    .route("/projects/{project}/tasks/{task}", get(show).patch(patch))
    .route("/projects/{project}/tasks/{task}/history", get(history))
}

async fn list(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
) -> Result<Json<Tasks>, ServerError> {
  let response = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      Ok(Tasks {
        tasks: task_rows::ids(db, project)?
          .into_iter()
          .map(|id| task_rows::load(db, project, id, now))
          .collect::<Result<_, _>>()?,
        cursor: events::cursor(db, project)?,
      })
    })
    .await?;
  Ok(Json(response))
}

async fn show(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
) -> Result<Json<TaskResponse>, ServerError> {
  let task = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      task_rows::load(db, project, task, now)
    })
    .await?;
  Ok(Json(TaskResponse { task }))
}

async fn create(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<TasksCreate>,
) -> Result<(StatusCode, Json<TasksCreated>), ServerError> {
  let response = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      batches::create(db, project, current.user.id, input.idempotency_key, input.tasks, now)
    })
    .await?;
  Ok((StatusCode::CREATED, Json(response)))
}

async fn import(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id(project): Id<ProjectId>,
  SafeJson(input): SafeJson<TasksImport>,
) -> Result<(StatusCode, Json<TasksCreated>), ServerError> {
  let response = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      batches::import(
        db,
        project,
        current.user.id,
        input.idempotency_key,
        (input.tasks, input.modules),
        now,
      )
    })
    .await?;
  Ok((StatusCode::CREATED, Json(response)))
}

async fn patch(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
  SafeJson(input): SafeJson<TaskPatch>,
) -> Result<Json<TaskResponse>, ServerError> {
  let task = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      task_write::patch(db, project, task, current.user.id, input, now)
    })
    .await?;
  Ok(Json(TaskResponse { task }))
}

async fn history(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
) -> Result<Json<OrganizationHistory>, ServerError> {
  let response = server
    .snapshot(move |db, now| {
      access::member(db, &auth, project, now)?;
      task_rows::load(db, project, task, now)?;
      organization::history(db, task)
    })
    .await?;
  Ok(Json(response))
}
