use orchester_api::{ErrorCode, LostClaim, SharedClaim, SharedTask, SharingErrorDetails};
use orchester_core::{
  id::{ClaimId, ProjectId, TaskId, UserId},
  run::TaskPhase,
  sharing::{Claim, ClaimError, ClaimKind},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use super::{access, conflict, events, reason, task_rows, task_write};
use crate::{db, error::ServerError};

pub fn snapshot(db: &Connection, claim: &Claim, now: i64) -> Result<SharedClaim, ServerError> {
  Ok(SharedClaim {
    id: claim.id(),
    holder: access::user_ref(db, claim.holder())?,
    kind: claim.kind(),
    since: claim.since().unix_millis(),
    heartbeat_at: claim.heartbeat_at().unix_millis(),
    stale: claim.stale(Timestamp::from_unix_millis(now)),
    taken_over_from: claim.taken_over_from().map(|id| access::user_ref(db, id)).transpose()?,
  })
}

fn failure(db: &Connection, error: ClaimError, claim: Option<&Claim>, now: i64) -> Result<ServerError, ServerError> {
  let code = match error {
    ClaimError::Held => ErrorCode::ClaimHeld,
    ClaimError::Merged => return Ok(ErrorCode::TaskMerged.into()),
    ClaimError::NotHolder => ErrorCode::NotHolder,
    ClaimError::Changed => ErrorCode::ClaimChanged,
    ClaimError::NotStale => ErrorCode::ClaimNotStale,
    ClaimError::AgentWorking => return Ok(reason(ErrorCode::NotEditable, "stop_run_first")),
  };
  Ok(conflict(
    code,
    SharingErrorDetails::Claim {
      claim: claim.map(|claim| snapshot(db, claim, now)).transpose()?,
    },
  ))
}

pub fn save(db: &Connection, task: TaskId, claim: &Claim) -> Result<(), ServerError> {
  db.execute("INSERT INTO claims(id,task,holder,kind,since,heartbeat_at,taken_over_from) VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(task) DO UPDATE SET id=excluded.id,holder=excluded.holder,kind=excluded.kind,since=excluded.since,heartbeat_at=excluded.heartbeat_at,taken_over_from=excluded.taken_over_from",
    params![claim.id().to_string(),task.to_string(),claim.holder().to_string(),match claim.kind() {ClaimKind::Manual=>"manual",ClaimKind::Run=>"run"},claim.since().unix_millis(),claim.heartbeat_at().unix_millis(),claim.taken_over_from().map(|id|id.to_string())])?;
  if claim.kind() == ClaimKind::Run {
    db.execute("UPDATE tasks SET has_run=1 WHERE id=?1", [task.to_string()])?;
  }
  Ok(())
}

#[derive(Clone, Copy)]
enum ClaimAction {
  Taken,
  Released,
  TakenOver,
}

fn record(
  db: &Connection,
  task: TaskId,
  claim: &Claim,
  action: ClaimAction,
  user: UserId,
  now: i64,
) -> Result<(), ServerError> {
  let (action, from) = match action {
    ClaimAction::Taken => ("taken", None),
    ClaimAction::Released => ("released", None),
    ClaimAction::TakenOver => ("taken_over", claim.taken_over_from()),
  };
  db.execute(
    "INSERT INTO claim_events(task,claim,action,user_id,from_user,at) VALUES (?1,?2,?3,?4,?5,?6)",
    params![
      task.to_string(),
      claim.id().to_string(),
      action,
      user.to_string(),
      from.map(|id| id.to_string()),
      now
    ],
  )?;
  Ok(())
}

pub fn take(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  user: UserId,
  kind: ClaimKind,
  now: i64,
) -> Result<SharedTask, ServerError> {
  let old = task_rows::load(db, project, task, now)?;
  let current = task_rows::claim(db, task)?;
  let next = match Claim::take(
    current.as_ref(),
    ClaimId::generate(),
    user,
    kind,
    Timestamp::from_unix_millis(now),
    old.run.as_ref().is_some_and(|run| run.phase == TaskPhase::Merged) || completed(db, task)?,
  ) {
    Ok(claim) => claim,
    Err(error) => return Err(failure(db, error, current.as_ref(), now)?),
  };
  if current.as_ref() == Some(&next) {
    return Ok(old);
  }
  save(db, task, &next)?;
  record(db, task, &next, ClaimAction::Taken, user, now)?;
  task_write::bump(db, task, user, now)?;
  events::task(db, project, task, now)?;
  task_rows::load(db, project, task, now)
}

pub fn upload(db: &Connection, task: TaskId, user: UserId, now: i64) -> Result<(), ServerError> {
  let claim = Claim::take(
    None,
    ClaimId::generate(),
    user,
    ClaimKind::Run,
    Timestamp::from_unix_millis(now),
    false,
  )
  .map_err(|_| ServerError::StoredData)?;
  save(db, task, &claim)?;
  record(db, task, &claim, ClaimAction::Taken, user, now)
}

fn completed(db: &Connection, task: TaskId) -> Result<bool, ServerError> {
  Ok(db.query_row(
    "SELECT EXISTS(SELECT 1 FROM claim_events WHERE task=?1 AND action='completed')",
    [task.to_string()],
    |row| row.get(0),
  )?)
}

pub fn release(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  user: UserId,
  now: i64,
) -> Result<SharedTask, ServerError> {
  task_rows::load(db, project, task, now)?;
  let claim = task_rows::claim(db, task)?;
  let Some(claim) = claim else {
    return Err(failure(db, ClaimError::NotHolder, None, now)?);
  };
  if let Err(error) = claim.check_release(user, task_rows::run(db, task)?.map(|run| run.phase)) {
    return Err(failure(db, error, Some(&claim), now)?);
  }
  forced_release(db, project, task, &claim, user, now)?;
  task_rows::load(db, project, task, now)
}

fn forced_release(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  claim: &Claim,
  actor: UserId,
  now: i64,
) -> Result<(), ServerError> {
  db.execute("DELETE FROM claims WHERE task=?1", [task.to_string()])?;
  db.execute("DELETE FROM run_status WHERE task=?1", [task.to_string()])?;
  record(db, task, claim, ClaimAction::Released, actor, now)?;
  task_write::bump(db, task, actor, now)?;
  events::task(db, project, task, now)
}

pub fn release_user(
  db: &Connection,
  project: Option<ProjectId>,
  user: UserId,
  actor: UserId,
  now: i64,
) -> Result<(), ServerError> {
  let mut query = db.prepare("SELECT c.task,t.project FROM claims c JOIN tasks t ON t.id=c.task WHERE c.holder=?1 AND (?2 IS NULL OR t.project=?2)")?;
  let ids = query
    .query_map(params![user.to_string(), project.map(|id| id.to_string())], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  for (task, project) in ids {
    let task = task.parse().map_err(|_| ServerError::StoredData)?;
    let project = project.parse().map_err(|_| ServerError::StoredData)?;
    let claim = task_rows::claim(db, task)?.ok_or(ServerError::StoredData)?;
    forced_release(db, project, task, &claim, actor, now)?;
  }
  Ok(())
}

pub fn takeover(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  user: UserId,
  seen: ClaimId,
  now: i64,
) -> Result<SharedTask, ServerError> {
  task_rows::load(db, project, task, now)?;
  let current = task_rows::claim(db, task)?;
  let Some(current) = current else {
    return Err(failure(db, ClaimError::Changed, None, now)?);
  };
  let next = match current.take_over(seen, ClaimId::generate(), user, Timestamp::from_unix_millis(now)) {
    Ok(next) => next,
    Err(error) => return Err(failure(db, error, Some(&current), now)?),
  };
  save(db, task, &next)?;
  record(db, task, &next, ClaimAction::TakenOver, user, now)?;
  db::audit(db, Some(user), "claim_taken_over", &task.to_string(), None, now)?;
  task_write::bump(db, task, user, now)?;
  events::task(db, project, task, now)?;
  task_rows::load(db, project, task, now)
}

pub fn heartbeat(db: &Connection, user: UserId, tasks: Vec<TaskId>, now: i64) -> Result<Vec<LostClaim>, ServerError> {
  let mut lost = Vec::new();
  for task in tasks.into_iter().collect::<std::collections::BTreeSet<_>>() {
    let Some(project) = task_rows::project_of(db, task)? else {
      lost.push(LostClaim { task, claim: None });
      continue;
    };
    if access::role(db, project, user)?.is_none() {
      lost.push(LostClaim { task, claim: None });
      continue;
    }
    let current = task_rows::claim(db, task)?;
    let Some(mut claim) = current else {
      lost.push(LostClaim { task, claim: None });
      continue;
    };
    if claim.heartbeat(user, Timestamp::from_unix_millis(now)).is_err() {
      lost.push(LostClaim {
        task,
        claim: Some(snapshot(db, &claim, now)?),
      });
      continue;
    }
    save(db, task, &claim)?;
    events::task(db, project, task, now)?;
  }
  Ok(lost)
}
