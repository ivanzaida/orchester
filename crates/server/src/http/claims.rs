use axum::{Json, Router, extract::State, routing::post};
use orchester_api::*;
use orchester_core::id::{ProjectId, TaskId};

use super::extract::{Auth, Id, SafeJson};
use crate::{
  Server, db,
  error::ServerError,
  sharing::{access, claims},
};

pub fn routes() -> Router<Server> {
  Router::new()
    .route("/projects/{project}/tasks/{task}/claim", post(take).delete(release))
    .route("/projects/{project}/tasks/{task}/claim/take-over", post(takeover))
    .route("/heartbeat", post(heartbeat))
}

async fn take(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
  SafeJson(input): SafeJson<ClaimTake>,
) -> Result<Json<TaskResponse>, ServerError> {
  let task = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      claims::take(db, project, task, current.user.id, input.kind, now)
    })
    .await?;
  Ok(Json(TaskResponse { task }))
}

async fn release(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
) -> Result<Json<TaskResponse>, ServerError> {
  let task = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      claims::release(db, project, task, current.user.id, now)
    })
    .await?;
  Ok(Json(TaskResponse { task }))
}

async fn takeover(
  State(server): State<Server>,
  Auth(auth): Auth,
  Id((project, task)): Id<(ProjectId, TaskId)>,
  SafeJson(input): SafeJson<ClaimTakeOver>,
) -> Result<Json<TaskResponse>, ServerError> {
  let task = server
    .write(move |db, now| {
      let current = access::member(db, &auth, project, now)?;
      claims::takeover(db, project, task, current.user.id, input.claim, now)
    })
    .await?;
  Ok(Json(TaskResponse { task }))
}

async fn heartbeat(
  State(server): State<Server>,
  Auth(auth): Auth,
  SafeJson(input): SafeJson<Heartbeat>,
) -> Result<Json<HeartbeatResponse>, ServerError> {
  if input.tasks.len() > crate::sharing::limits::HEARTBEAT_TASK_LIMIT {
    return Err(ErrorCode::Invalid.into());
  }
  server.inner.sharing_limits.heartbeat(auth.session, server.now())?;
  let lost = server
    .write(move |db, now| {
      let current = db::checked(db, &auth, now, false)?;
      claims::heartbeat(db, current.user.id, input.tasks, now)
    })
    .await?;
  Ok(Json(HeartbeatResponse { lost }))
}
