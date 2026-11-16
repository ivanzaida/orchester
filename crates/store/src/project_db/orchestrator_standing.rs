//! The events of a target a conversation leads to the end (`orchester_core::orchestrator::standing`): the state last
//! seen of each leading subscription, and the event kept in its conversation when that state changed to one that
//! raises one. An event is a `report` entry whose body is `{"kind": "standing", …}`, as a subscription's end is
//! (`super::orchestrator_watch_summaries`): it gets its turn as a report does, and unlike one its turn may act.

use orchester_core::{
  orchestrator::{Subscription, WatchTarget, standing::StandingEvent},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

const STANDING: &str = "standing";

/// An event's body as its entry keeps it.
#[derive(Serialize, Deserialize)]
struct Kept {
  kind: String,
  #[serde(flatten)]
  event: StandingEvent,
}

/// The event a `report` entry's `body` holds; `None` for a daily report or a subscription's end.
pub(super) fn standing_of(body: &str) -> Result<Option<StandingEvent>, StoreError> {
  let value: serde_json::Value = serde_json::from_str(body).map_err(|error| corrupt("daily report", error))?;
  if value.get("kind").and_then(serde_json::Value::as_str) != Some(STANDING) {
    return Ok(None);
  }
  let kept: Kept = serde_json::from_value(value).map_err(|error| corrupt("standing event", error))?;
  Ok(Some(kept.event))
}

fn target_key(target: WatchTarget) -> String {
  match target {
    WatchTarget::Task(task) => format!("task:{task}"),
    WatchTarget::Module(module) => format!("module:{module}"),
    WatchTarget::WithoutModule => "without_module".to_owned(),
    WatchTarget::Project => "project".to_owned(),
  }
}

impl ProjectDb {
  /// The state last seen of what `subscription` leads; `None` before the first.
  pub fn standing_state(&self, subscription: &Subscription) -> Result<Option<String>, StoreError> {
    Ok(
      self
        .connection
        .query_row(
          "SELECT state FROM orchestrator_standings WHERE chat_id = ?1 AND target = ?2 AND since = ?3",
          params![
            subscription.chat().to_string(),
            target_key(subscription.target()),
            subscription.since().unix_millis()
          ],
          |row| row.get(0),
        )
        .optional()?,
    )
  }

  /// Keeps `state` as the one last seen of what `subscription` leads and, with it, `event` in its conversation at
  /// `at` when the state raises one, in one transaction.
  pub fn keep_standing(
    &mut self,
    subscription: &Subscription,
    (state, event): (&str, Option<&StandingEvent>),
    at: Timestamp,
  ) -> Result<(), StoreError> {
    let (chat, target) = (subscription.chat().to_string(), target_key(subscription.target()));
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO orchestrator_standings (chat_id, target, since, state) VALUES (?1, ?2, ?3, ?4)
       ON CONFLICT (chat_id, target, since) DO UPDATE SET state = excluded.state",
      params![chat, target, subscription.since().unix_millis(), state],
    )?;
    if let Some(event) = event {
      let body = serde_json::to_string(&Kept {
        kind: STANDING.to_owned(),
        event: event.clone(),
      })
      .map_err(|error| corrupt("standing event", error))?;
      transaction.execute(
        "INSERT INTO orchestrator_entries (chat_id, kind, at, body, wait_key) VALUES (?1, 'report', ?2, ?3, ?4)
         ON CONFLICT (wait_key) DO NOTHING",
        params![
          chat,
          at.unix_millis(),
          body,
          format!("standing:{chat}:{target}:{}", at.unix_millis())
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }
}
