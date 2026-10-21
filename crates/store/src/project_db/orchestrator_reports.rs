//! The daily reports of the project in the Orchestrator chat (`orchester_core::orchestrator::report`): each kept once,
//! by the key of its day ([`DailyReport::key_of`]), in the conversation the app chose; a report that starts a turn by
//! itself gets it as a wait does ([`ProjectDb::start_wait_turn`]).

use orchester_core::{
  id::ChatId,
  orchestrator::{Chat, DailyReport},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A report kept without a turn, which starts one by itself, and the conversation it was kept in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptReport {
  pub chat: ChatId,
  pub seq: i64,
}

impl ProjectDb {
  /// Keeps `report` in `chat` (stored with it when it is new) at `at`; whether it was new: a day's report is kept
  /// once, whichever conversation kept it.
  pub fn keep_daily_report(&mut self, chat: &Chat, report: &DailyReport, at: Timestamp) -> Result<bool, StoreError> {
    let body = serde_json::to_string(report).map_err(|error| corrupt("daily report", error))?;
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO orchestrator_chats (id, title, started_at) VALUES (?1, ?2, ?3) ON CONFLICT (id) DO NOTHING",
      params![chat.id().to_string(), chat.title(), chat.started().unix_millis()],
    )?;
    let kept = transaction.execute(
      "INSERT INTO orchestrator_entries (chat_id, kind, at, body, wait_key) VALUES (?1, 'report', ?2, ?3, ?4)
       ON CONFLICT (wait_key) DO NOTHING",
      params![
        chat.id().to_string(),
        at.unix_millis(),
        body,
        DailyReport::key_of(report.day)
      ],
    )?;
    transaction.commit()?;
    Ok(kept == 1)
  }

  /// The report kept by `key` ([`DailyReport::key_of`]); `None` while that day has none.
  pub fn daily_report(&self, key: &str) -> Result<Option<DailyReport>, StoreError> {
    let body = self
      .connection
      .query_row(
        "SELECT body FROM orchestrator_entries WHERE kind = 'report' AND wait_key = ?1",
        [key],
        |row| row.get::<_, String>(0),
      )
      .optional()?;
    body.map(|body| report_of(&body)).transpose()
  }

  /// The reports of every conversation that start a turn by themselves (`automatic`) and have none, oldest first.
  pub fn automatic_reports_without_turn(&self) -> Result<Vec<KeptReport>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT chat_id, seq FROM orchestrator_entries
       WHERE kind = 'report' AND message_id IS NULL AND json_extract(body, '$.automatic') = 1 ORDER BY seq",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(chat, seq)| {
        Ok(KeptReport {
          chat: parse("chat id", &chat)?,
          seq,
        })
      })
      .collect()
  }
}

/// The report a `report` entry's body holds.
pub(super) fn report_of(body: &str) -> Result<DailyReport, StoreError> {
  serde_json::from_str(body).map_err(|error| corrupt("daily report", error))
}
