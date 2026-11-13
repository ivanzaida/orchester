//! The program of each agent CLI on this computer (`docs/design/agent-accounts.md`, "Agents run on accounts"): the
//! person sets the path once and every project's agents of that CLI start it. A CLI without a row is looked up on
//! the `PATH`. A path whose file is gone stays stored; whoever shows it marks it.

use std::path::{Path, PathBuf};

use orchester_core::{account::AccountCli, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, path_text},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE agent_programs (
  cli TEXT PRIMARY KEY CHECK (cli IN ('claude_code', 'codex')),
  executable TEXT NOT NULL,
  set_at INTEGER NOT NULL
) STRICT;
";

impl AppDb {
  /// Keeps `executable` as the program of `cli`, in place of an earlier one.
  pub fn set_agent_program(&mut self, cli: AccountCli, executable: &Path, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO agent_programs (cli, executable, set_at) VALUES (?1, ?2, ?3)
       ON CONFLICT (cli) DO UPDATE SET executable = excluded.executable, set_at = excluded.set_at",
      params![
        cli.as_str(),
        path_text("agent CLI program", executable)?,
        at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Forgets the program set for `cli`, which is looked up on the `PATH` again; whether there was one.
  pub fn clear_agent_program(&mut self, cli: AccountCli) -> Result<bool, StoreError> {
    let removed = self
      .connection
      .execute("DELETE FROM agent_programs WHERE cli = ?1", [cli.as_str()])?;
    Ok(removed > 0)
  }

  /// The program set for `cli`, if any. The file may be gone.
  pub fn agent_program(&self, cli: AccountCli) -> Result<Option<PathBuf>, StoreError> {
    let executable: Option<String> = self
      .connection
      .query_row(
        "SELECT executable FROM agent_programs WHERE cli = ?1",
        [cli.as_str()],
        |row| row.get(0),
      )
      .optional()?;
    match executable {
      Some(path) if path.is_empty() => Err(corrupt("agent CLI program", "empty path")),
      Some(path) => Ok(Some(PathBuf::from(path))),
      None => Ok(None),
    }
  }
}
