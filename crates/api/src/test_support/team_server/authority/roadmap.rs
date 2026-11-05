//! The stand-in's releases (`docs/cloud-api.md`, "Team authority releases"): an Admin's writes of a release's
//! definition on the `/api/v1` routes (create, change, *Mark released* with the moves it makes, delete of a planned
//! one, the changelog), each kept by its idempotency key and answered again with its first answer; a task's move into
//! or out of a release in the task's change ([`super::edits`]); and the `/api/v2` reads ([`reads`]). A member's write
//! is `403 not_admin`. Releases are made by these writes only: none is set up beforehand.

mod reads;

use std::collections::HashMap;

use orchester_core::{
  id::{IdempotencyKey, ReleaseId, TaskId},
  run::CommitId,
  sharing::TaskRevision,
  task::organization::ChangeActor,
  time::Timestamp,
  usage::local_date,
};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

pub(super) use self::reads::read;
use super::{Authority, Person, Standing, edits, listing};
use crate::{
  Attribution, ChangelogDocument, ChangelogPatch, KeptEntry, MarkReleased, ReleaseChange, ReleaseCreate, ReleaseDelete,
  ReleaseMark, ReleasePatch, ReleaseRefusal, SharedChangelog, SharedRelease, UserRef,
  test_support::team_server::{State, USER},
};

/// A release with its changelog.
struct Kept {
  release: SharedRelease,
  document: ChangelogDocument,
  hidden: Vec<TaskId>,
  kept_aside: Vec<KeptEntry>,
}

impl Kept {
  fn changelog(&self) -> SharedChangelog {
    SharedChangelog {
      release: self.release.id,
      revision: self.release.revision,
      document: self.document.clone(),
      hidden: self.hidden.clone(),
      kept_aside: self.kept_aside.clone(),
    }
  }
}

/// The project's releases in the order they were created, every move of a task into or out of one, and the first
/// answer of each write by its idempotency key.
#[derive(Default)]
pub(super) struct Releases {
  kept: Vec<Kept>,
  changes: Vec<ReleaseChange>,
  answered: HashMap<IdempotencyKey, (u16, Value)>,
}

impl Releases {
  fn get(&self, id: ReleaseId) -> Option<&Kept> {
    self.kept.iter().find(|kept| kept.release.id == id)
  }

  fn get_mut(&mut self, id: ReleaseId) -> Option<&mut Kept> {
    self.kept.iter_mut().find(|kept| kept.release.id == id)
  }

  /// The release other than `but` named `name`, without regard to case.
  fn named(&self, name: &str, but: Option<ReleaseId>) -> Option<&SharedRelease> {
    let name = name.trim().to_lowercase();
    self
      .kept
      .iter()
      .map(|kept| &kept.release)
      .find(|release| Some(release.id) != but && release.fields.name.trim().to_lowercase() == name)
  }

  /// The planned release after `id` in the releases' order: those with a planned day by day, then those without one
  /// in the order they were created; `id` itself left out.
  fn next_planned(&self, id: ReleaseId) -> Option<ReleaseId> {
    let mut planned: Vec<&SharedRelease> = self
      .kept
      .iter()
      .map(|kept| &kept.release)
      .filter(|release| release.released.is_none())
      .collect();
    planned.sort_by_key(|release| (release.fields.planned.is_none(), release.fields.planned.clone()));
    let at = planned.iter().position(|release| release.id == id)?;
    planned.get(at + 1).map(|release| release.id)
  }

  /// Records the move of `task` from `from` to `to` by the caller, as `actor`.
  fn moved(
    &mut self,
    task: TaskId,
    (from, to): (Option<ReleaseId>, Option<ReleaseId>),
    actor: ChangeActor,
  ) -> ReleaseChange {
    let change = ReleaseChange {
      sequence: self.changes.len() as u64 + 1,
      task,
      from,
      to,
      by: you(),
      actor,
      at: Timestamp::now().unix_millis(),
    };
    self.changes.push(change.clone());
    change
  }
}

/// A write of a release on `path` (method first) with `body`: the status line and the answer; `None` for another
/// request.
pub(in crate::test_support::team_server) fn answer(
  path: &str,
  body: &str,
  state: &Mutex<State>,
) -> Option<(&'static str, Value)> {
  let (method, rest) = path.split_once(' ')?;
  let (project, rest) = rest.strip_prefix("/api/v1/projects/")?.split_once("/releases")?;
  let mut state = state.lock();
  let key = state
    .projects
    .iter()
    .find(|known| known["id"] == project)
    .and_then(|known| known["key"].as_str())
    .map(str::to_owned);
  let (status, answer) = match key {
    None => (404, error("not_found")),
    Some(_) if state.authority.standing == Standing::Member => (403, error("not_admin")),
    Some(key) => write(
      &mut state,
      (project, &key),
      (method, rest.trim_start_matches('/')),
      body,
    ),
  };
  let line = match status {
    200 => "200 OK",
    201 => "201 Created",
    400 => "400 Bad Request",
    403 => "403 Forbidden",
    404 => "404 Not Found",
    409 => "409 Conflict",
    _ => "422 Unprocessable Entity",
  };
  Some((line, answer))
}

fn write(state: &mut State, project: (&str, &str), (method, rest): (&str, &str), body: &str) -> (u16, Value) {
  let (release, part) = rest.split_once('/').unwrap_or((rest, ""));
  if (method, release) == ("POST", "") {
    return create(state, body);
  }
  let Ok(release) = release.parse::<ReleaseId>() else {
    return (404, error("not_found"));
  };
  match (method, part) {
    ("PATCH", "") => patch(state, release, body),
    ("POST", "released") => mark(state, project, release, body),
    ("DELETE", "") => delete(state, project, release, body),
    ("PATCH", "changelog") => changelog(state, release, body),
    _ => (404, error("not_found")),
  }
}

/// `body` as `T`, or `400 bad_request` for a body that is not one, an unknown field too.
fn parse<T: DeserializeOwned>(body: &str) -> Result<T, (u16, Value)> {
  serde_json::from_str(body).map_err(|_| (400, error("bad_request")))
}

/// The first answer under `key`, or `write`'s, kept as it.
fn once(state: &mut State, key: IdempotencyKey, write: impl FnOnce(&mut State) -> (u16, Value)) -> (u16, Value) {
  if let Some(answered) = state.authority.releases.answered.get(&key) {
    return answered.clone();
  }
  let answer = write(state);
  state.authority.releases.answered.insert(key, answer.clone());
  answer
}

fn create(state: &mut State, body: &str) -> (u16, Value) {
  let create: ReleaseCreate = match parse(body) {
    Ok(create) => create,
    Err(refusal) => return refusal,
  };
  once(state, create.idempotency_key, |state| {
    if create.release.details().is_err() {
      return (422, error("invalid"));
    }
    let releases = &mut state.authority.releases;
    if releases.get(create.id).is_some() {
      return (409, error("id_taken"));
    }
    if let Some(taken) = releases.named(&create.release.name, None) {
      return (409, refusal("release_exists", "release", json!(taken)));
    }
    let release = SharedRelease {
      id: create.id,
      fields: create.release,
      released: None,
      revision: TaskRevision::FIRST,
      created: now(),
      updated: now(),
    };
    releases.kept.push(Kept {
      release: release.clone(),
      document: ChangelogDocument::empty(),
      hidden: Vec::new(),
      kept_aside: Vec::new(),
    });
    state.authority.commit(vec![json!({"kind": "release_list"})]);
    (201, json!({"release": release}))
  })
}

/// The release `id` as a write made on `revision` finds it: still planned and at that revision, or the refusal.
fn writable(authority: &Authority, id: ReleaseId, revision: TaskRevision) -> Result<(), (u16, Value)> {
  let Some(kept) = authority.releases.get(id) else {
    return Err((404, error("not_found")));
  };
  if kept.release.revision != revision {
    return Err((409, refusal("revision_conflict", "current", json!(kept.release))));
  }
  if kept.release.released.is_some() {
    return Err(release_refused(ReleaseRefusal::Released));
  }
  Ok(())
}

/// The release `id` changed by `change`, at its next revision in the caller's name; committed with `scopes`.
fn changed(
  authority: &mut Authority,
  id: ReleaseId,
  change: impl FnOnce(&mut Kept),
  scopes: Vec<Value>,
) -> Option<&Kept> {
  let kept = authority.releases.get_mut(id)?;
  change(kept);
  kept.release.revision = kept.release.revision.next().ok()?;
  kept.release.updated = now();
  let mut scopes = scopes;
  scopes.extend([
    json!({"kind": "release_list"}),
    json!({"kind": "release", "release": id}),
  ]);
  authority.commit(scopes);
  authority.releases.get(id)
}

fn patch(state: &mut State, id: ReleaseId, body: &str) -> (u16, Value) {
  let patch: ReleasePatch = match parse(body) {
    Ok(patch) => patch,
    Err(refusal) => return refusal,
  };
  once(state, patch.idempotency_key, |state| {
    if let Err(refusal) = writable(&state.authority, id, patch.revision) {
      return refusal;
    }
    if patch.release.details().is_err() {
      return (422, error("invalid"));
    }
    if let Some(taken) = state.authority.releases.named(&patch.release.name, Some(id)) {
      return (409, refusal("release_exists", "release", json!(taken)));
    }
    let fields = patch.release;
    match changed(
      &mut state.authority,
      id,
      |kept| kept.release.fields = fields,
      Vec::new(),
    ) {
      Some(kept) => (200, json!({"release": kept.release})),
      None => (404, error("not_found")),
    }
  })
}

/// *Mark released*: the release's tasks that are neither Done nor Canceled move to the next planned release, or out of
/// any; the release records today, the caller and the commit.
fn mark(state: &mut State, project: (&str, &str), id: ReleaseId, body: &str) -> (u16, Value) {
  let mark: MarkReleased = match parse(body) {
    Ok(mark) => mark,
    Err(refusal) => return refusal,
  };
  once(state, mark.idempotency_key, |state| {
    if let Err(refusal) = writable(&state.authority, id, mark.revision) {
      return refusal;
    }
    let next = state.authority.releases.next_planned(id);
    let unfinished = |task: &Value| task["workflow"] != "done" && task["workflow"] != "canceled";
    let moves = move_tasks(state, project, (id, next), (unfinished, ChangeActor::Orchester));
    let commit: Option<CommitId> = mark.commit;
    let released = |kept: &mut Kept| {
      let on = local_date(Timestamp::now()).format("%Y-%m-%d").to_string();
      kept.release.released = Some(ReleaseMark { on, by: you(), commit });
    };
    let scopes = next
      .map(|next| json!({"kind": "release", "release": next}))
      .into_iter()
      .collect();
    match changed(&mut state.authority, id, released, scopes) {
      Some(kept) => (200, json!({"release": kept.release, "moves": moves})),
      None => (404, error("not_found")),
    }
  })
}

fn delete(state: &mut State, project: (&str, &str), id: ReleaseId, body: &str) -> (u16, Value) {
  let delete: ReleaseDelete = match parse(body) {
    Ok(delete) => delete,
    Err(refusal) => return refusal,
  };
  once(state, delete.idempotency_key, |state| {
    if let Err(refusal) = writable(&state.authority, id, delete.revision) {
      return refusal;
    }
    let moves = move_tasks(state, project, (id, None), (|_: &Value| true, ChangeActor::Person));
    state.authority.releases.kept.retain(|kept| kept.release.id != id);
    state.authority.commit(vec![
      json!({"kind": "release_list"}),
      json!({"kind": "release", "release": id}),
    ]);
    (200, json!({"moves": moves}))
  })
}

fn changelog(state: &mut State, id: ReleaseId, body: &str) -> (u16, Value) {
  let patch: ChangelogPatch = match parse(body) {
    Ok(patch) => patch,
    Err(refusal) => return refusal,
  };
  once(state, patch.idempotency_key, |state| {
    if let Err(refusal) = writable(&state.authority, id, patch.revision) {
      return refusal;
    }
    let ChangelogPatch {
      document,
      hidden,
      kept_aside,
      ..
    } = patch;
    let set = |kept: &mut Kept| {
      kept.document = document;
      kept.hidden = hidden;
      kept.kept_aside = kept_aside;
    };
    match changed(&mut state.authority, id, set, Vec::new()) {
      Some(kept) => (200, json!({"changelog": kept.changelog()})),
      None => (404, error("not_found")),
    }
  })
}

/// Moves each task of the release `from` that `which` keeps to `to`, as `actor`: the move goes into the task's change,
/// at its next revision, and into the releases' history.
fn move_tasks(
  state: &mut State,
  project: (&str, &str),
  (from, to): (ReleaseId, Option<ReleaseId>),
  (which, actor): (impl Fn(&Value) -> bool, ChangeActor),
) -> Vec<ReleaseChange> {
  let inside: Vec<Value> = listing::tasks(project, state)
    .into_iter()
    .filter(|task| release_of(task) == Some(from) && which(task))
    .collect();
  inside
    .iter()
    .filter_map(|task| {
      let id = task["id"].as_str()?.parse::<TaskId>().ok()?;
      let mut fields = Map::new();
      fields.insert("release".to_owned(), json!(to));
      edits::change(&mut state.authority, task, (fields, Person::You));
      Some(state.authority.releases.moved(id, (Some(from), to), actor))
    })
    .collect()
}

/// A task's change that moves it into or out of a release, `asked` as the change's wire: refused when it names a
/// release the project does not have, or a released one, or takes the task out of a released one.
pub(super) fn refused_move(authority: &Authority, current: &Value, asked: &Value) -> Option<(u16, Value)> {
  let to = asked.get("release")?;
  let releases = &authority.releases;
  let released = |id: Option<ReleaseId>| {
    id.and_then(|id| releases.get(id))
      .is_some_and(|kept| kept.release.released.is_some())
  };
  let to = match serde_json::from_value::<Option<ReleaseId>>(to.clone()) {
    Ok(Some(id)) if releases.get(id).is_none() => return Some(release_refused(ReleaseRefusal::ReleaseNotFound)),
    Ok(to) => to,
    Err(_) => return Some((422, error("invalid"))),
  };
  (released(to) || released(release_of(current))).then(|| release_refused(ReleaseRefusal::Released))
}

/// Records the move a task's change made, `current` being the task before it; nothing when it moved none.
pub(super) fn record_move(authority: &mut Authority, current: &Value, asked: &Value, actor: ChangeActor) {
  let Some(to) = asked.get("release") else {
    return;
  };
  let (from, to) = (release_of(current), serde_json::from_value(to.clone()).ok().flatten());
  let Some(task) = current["id"].as_str().and_then(|id| id.parse::<TaskId>().ok()) else {
    return;
  };
  if from != to {
    authority.releases.moved(task, (from, to), actor);
    let scopes = [from, to]
      .into_iter()
      .flatten()
      .map(|id| json!({"kind": "release", "release": id}));
    authority.commit(scopes.collect());
  }
}

/// The release a task read names.
fn release_of(task: &Value) -> Option<ReleaseId> {
  task["release"].as_str().and_then(|id| id.parse().ok())
}

fn you() -> UserRef {
  UserRef {
    id: USER.parse().expect("the stand-in's user is a user id"),
    display_name: "Bohdan Koval".to_owned(),
  }
}

fn now() -> Attribution {
  Attribution {
    by: you(),
    at: Timestamp::now().unix_millis(),
  }
}

fn error(code: &str) -> Value {
  json!({"error": code, "message": code})
}

/// `code`'s refusal with `value` under `field`.
fn refusal(code: &str, field: &str, value: Value) -> Value {
  let mut answer = error(code);
  answer[field] = value;
  answer
}

fn release_refused(reason: ReleaseRefusal) -> (u16, Value) {
  (422, refusal("release", "reason", json!(reason.as_str())))
}
