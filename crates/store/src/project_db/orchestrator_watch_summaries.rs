//! What a subscription that ended by itself leaves in its conversation (ORC-764, `orchester_core::orchestrator::
//! watch::WatchSummary`): its line, kept with the end in one transaction ([`ProjectDb::end_subscription_by_itself`]),
//! and the turn it starts by itself, which it gets as a daily report does ([`ProjectDb::start_wait_turn`]). It is a
//! `report` entry whose body is `{"kind": "watch_ended", …}`: the entries' kinds are a check, which cannot change in
//! place, and a report's turn is what this one is, one that changes nothing.

use orchester_core::{
  orchestrator::{Subscription, watch::WatchSummary},
  time::Timestamp,
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::{StoreError, rows::corrupt};

const WATCH_ENDED: &str = "watch_ended";

/// A summary's body as its entry keeps it.
#[derive(Serialize, Deserialize)]
struct Kept {
  kind: String,
  #[serde(flatten)]
  summary: WatchSummary,
}

/// The summary a `report` entry's `body` holds; `None` for a daily report.
pub(super) fn summary_of(body: &str) -> Result<Option<WatchSummary>, StoreError> {
  let value: serde_json::Value = serde_json::from_str(body).map_err(|error| corrupt("daily report", error))?;
  if value.get("kind").and_then(serde_json::Value::as_str) != Some(WATCH_ENDED) {
    return Ok(None);
  }
  let kept: Kept = serde_json::from_value(value).map_err(|error| corrupt("watch summary", error))?;
  Ok(Some(kept.summary))
}

/// Keeps `summary`, the end of `subscription`, in its conversation at `at`, once per subscription; whether it was new.
pub(super) fn keep(
  connection: &Connection,
  subscription: &Subscription,
  summary: &WatchSummary,
  at: Timestamp,
) -> Result<bool, StoreError> {
  let body = serde_json::to_string(&Kept {
    kind: WATCH_ENDED.to_owned(),
    summary: summary.clone(),
  })
  .map_err(|error| corrupt("watch summary", error))?;
  let kept = connection.execute(
    "INSERT INTO orchestrator_entries (chat_id, kind, at, body, wait_key) VALUES (?1, 'report', ?2, ?3, ?4)
     ON CONFLICT (wait_key) DO NOTHING",
    params![
      subscription.chat().to_string(),
      at.unix_millis(),
      body,
      WatchSummary::key_of(subscription)
    ],
  )?;
  Ok(kept == 1)
}
