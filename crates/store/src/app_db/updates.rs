//! Settings › Updates of the desktop app: the release channel (Stable until changed), whether a downloaded update
//! installs by itself (on until turned off), and when Orchester Cloud last answered an update check.

use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use rusqlite::params;

use super::AppDb;
use crate::{StoreError, rows::corrupt};

pub(super) const MIGRATION: &str = r"
-- The app's updates (ORC-413): Stable and installing by itself until a person changes them; when the cloud last
-- answered an update check, in milliseconds since the Unix epoch (NULL before the first answer).
ALTER TABLE app_settings ADD COLUMN update_channel TEXT NOT NULL DEFAULT 'stable'
  CHECK (update_channel IN ('stable', 'beta'));
ALTER TABLE app_settings ADD COLUMN update_automatically INTEGER NOT NULL DEFAULT 1
  CHECK (update_automatically IN (0, 1));
ALTER TABLE app_settings ADD COLUMN update_checked_at INTEGER;
";

impl AppDb {
  pub fn update_settings(&self) -> Result<UpdateSettings, StoreError> {
    let (channel, automatic): (String, bool) = self.connection.query_row(
      "SELECT update_channel, update_automatically FROM app_settings WHERE id = 1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(UpdateSettings {
      channel: Channel::parse(&channel).ok_or_else(|| corrupt("update channel", channel))?,
      automatic,
    })
  }

  pub fn set_update_channel(&mut self, channel: Channel) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET update_channel = ?1 WHERE id = 1",
      params![channel.as_str()],
    )?;
    Ok(())
  }

  pub fn set_update_automatically(&mut self, automatic: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET update_automatically = ?1 WHERE id = 1",
      params![automatic],
    )?;
    Ok(())
  }

  /// When Orchester Cloud last answered an update check from here; `None` before it ever did.
  pub fn update_checked_at(&self) -> Result<Option<Timestamp>, StoreError> {
    let at: Option<i64> =
      self
        .connection
        .query_row("SELECT update_checked_at FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?;
    Ok(at.map(Timestamp::from_unix_millis))
  }

  pub fn set_update_checked_at(&mut self, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET update_checked_at = ?1 WHERE id = 1",
      params![at.unix_millis()],
    )?;
    Ok(())
  }
}
