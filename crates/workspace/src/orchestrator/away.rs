//! *While you were away* (`docs/plan/orchestrator-autonomy/OA-23-…`): what a person who comes back reads first on the
//! project's Overview. The tasks the Orchestrator parked for them, each with what is needed, why it could not go on
//! and what to do, in its own words ([`super::parking`]); and the decisions it made by itself that they have not
//! looked at, newest first, each with its reason ([`super::decisions`]). A local project and a team project read the
//! same: both are this computer's records. Every function blocks on SQLite; the UI calls them off the UI thread.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  id::TaskId,
  orchestrator::{
    decision::{Decision, Review},
    parking::Parking,
  },
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb, StoreError};

use crate::messages::{self, Message};

/// A task parked for the person, named as people read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParkedTask {
  pub parking: Parking,
  /// The task's key (`KON-47`) and title; a task that is gone keeps its parking's id for a key.
  pub key: String,
  pub title: String,
}

/// A decision the person has not looked at, with the module of the task it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenDecision {
  pub decision: Decision,
  /// The module's name; `None` for a task in no module and a decision about no task.
  pub module: Option<String>,
}

/// What waits for a person who comes back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Away {
  /// Oldest first: what waited longest comes first.
  pub parked: Vec<ParkedTask>,
  /// Newest first.
  pub decisions: Vec<OpenDecision>,
}

impl Away {
  pub fn is_empty(&self) -> bool {
    self.parked.is_empty() && self.decisions.is_empty()
  }
}

/// What the project at `root` keeps for the person who comes back.
pub fn away_in(root: &Path) -> Result<Away, Message> {
  read(root).map_err(|error| messages::store_error(&error))
}

fn read(root: &Path) -> Result<Away, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  let tasks = db.tasks()?;
  let modules: HashMap<_, _> = db
    .modules()?
    .into_iter()
    .map(|module| (module.id(), module.name().as_str().to_owned()))
    .collect();
  let named: HashMap<TaskId, (String, String)> = tasks
    .iter()
    .map(|task| {
      (
        task.id(),
        (task.key().to_string(), task.details().title.as_str().to_owned()),
      )
    })
    .collect();
  let module_of = |key: &str| {
    let task = tasks
      .iter()
      .find(|task| task.key().to_string().eq_ignore_ascii_case(key))?;
    modules.get(&task.module()?).cloned()
  };
  let mut parked: Vec<ParkedTask> = db
    .parked_tasks()?
    .into_iter()
    .map(|parking| {
      let (key, title) = named
        .get(&parking.task)
        .cloned()
        .unwrap_or_else(|| (parking.task.to_string(), String::new()));
      ParkedTask { parking, key, title }
    })
    .collect();
  parked.sort_by_key(|parked| parked.parking.at);
  let mut decisions: Vec<OpenDecision> = db
    .decisions(&DecisionsRead {
      open_only: true,
      ..DecisionsRead::default()
    })?
    .into_iter()
    .map(|decision| OpenDecision {
      module: decision.made.task.as_deref().and_then(module_of),
      decision,
    })
    .collect();
  decisions.sort_by_key(|open| std::cmp::Reverse(open.decision.made.at));
  Ok(Away { parked, decisions })
}

/// Marks the decisions numbered `numbers` of the project at `root` as seen at `now`; one that is no longer open stays
/// as it is.
pub fn mark_seen_in(root: &Path, numbers: &[i64], now: Timestamp) -> Result<(), Message> {
  for number in numbers {
    super::decisions::review_in(root, *number, Review::Seen { at: now })?;
  }
  Ok(())
}

/// Marks decision `number` of the project at `root` as overruled at `now`: the person says so in a message of theirs
/// in the conversation that made it, which the caller opens with the decision quoted.
pub fn overrule_in(root: &Path, number: i64, now: Timestamp) -> Result<(), Message> {
  super::decisions::review_in(root, number, Review::Overruled { at: now }).map(|_| ())
}
