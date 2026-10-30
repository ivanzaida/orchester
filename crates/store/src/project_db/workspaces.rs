//! `workspaces`, `workspace_waits`, `workspace_settings`, `warm_folders` and `warm_up_failures` (ORC-114): the
//! project's warm workspaces and their settings. A borrow reads the workspaces and records its choice in one
//! immediate transaction, so the runner and the app never take the same one.

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  run::BranchName,
  time::Timestamp,
  workspaces::{
    Borrow, Holder, Stranded, WarmChoice, WarmFolder, WarmPattern, WarmUpCommand, WarmUpFailure, Warmth,
    WorkspaceNumber, WorkspaceRecord, WorkspaceSettings, WorkspaceStore, choose,
  },
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const COLUMNS: &str =
  "number, holder, last_used_at, last_run_id, branch, warmth, stranded_run_id, stranded_files FROM workspaces";

impl WorkspaceStore for ProjectDb {
  type Error = StoreError;

  fn workspaces(&self) -> Result<Vec<WorkspaceRecord>, StoreError> {
    records(&self.connection)
  }

  fn borrow_workspace(
    &mut self,
    holder: Holder,
    branch: Option<&BranchName>,
    limit: NonZeroU32,
    now: Timestamp,
  ) -> Result<Borrow, StoreError> {
    let transaction = self.connection.transaction()?;
    let borrow = choose(&records(&transaction)?, limit, holder);
    match borrow {
      Borrow::Take(number) => {
        transaction.execute(
          "INSERT INTO workspaces (number, holder, last_used_at, last_run_id, branch) VALUES (?1, ?2, ?3, ?4, ?5) \
           ON CONFLICT (number) DO UPDATE SET holder = excluded.holder, last_used_at = excluded.last_used_at, \
           last_run_id = coalesce(excluded.last_run_id, workspaces.last_run_id), branch = excluded.branch",
          params![
            number.get(),
            holder_text(holder)?,
            now.unix_millis(),
            holder.run().map(|run| run.to_string()),
            branch.map(BranchName::as_str),
          ],
        )?;
        if let Some(run) = holder.run() {
          transaction.execute("DELETE FROM workspace_waits WHERE run_id = ?1", [run.to_string()])?;
        }
      }
      Borrow::Wait(_) => {
        if let Some(run) = holder.run() {
          transaction.execute(
            "INSERT INTO workspace_waits (run_id, since) VALUES (?1, ?2) ON CONFLICT (run_id) DO NOTHING",
            params![run.to_string(), now.unix_millis()],
          )?;
        }
      }
    }
    transaction.commit()?;
    Ok(borrow)
  }

  fn return_workspace(&mut self, holder: Holder, now: Timestamp) -> Result<Option<WorkspaceNumber>, StoreError> {
    let number: Option<u32> = self
      .connection
      .query_row(
        "UPDATE workspaces SET holder = NULL, last_used_at = ?2, warmth = 'warm' WHERE holder = ?1 RETURNING number",
        params![holder_text(holder)?, now.unix_millis()],
        |row| row.get(0),
      )
      .optional()?;
    number.map(workspace_number).transpose()
  }

  fn return_runner_workspaces(&mut self, now: Timestamp) -> Result<Vec<WorkspaceNumber>, StoreError> {
    let transaction = self.connection.transaction()?;
    let mut returned = Vec::new();
    for record in records(&transaction)? {
      let Some(holder) = record.holder else {
        continue;
      };
      if matches!(holder, Holder::Recheck { .. } | Holder::Upkeep) {
        continue;
      }
      transaction.execute(
        "UPDATE workspaces SET holder = NULL, last_used_at = ?2 WHERE number = ?1",
        params![record.number.get(), now.unix_millis()],
      )?;
      returned.push(record.number);
    }
    transaction.commit()?;
    Ok(returned)
  }

  fn reserve_idle_workspace(&mut self, number: WorkspaceNumber, holder: Holder) -> Result<bool, StoreError> {
    let taken = self.connection.execute(
      "UPDATE workspaces SET holder = ?2 WHERE number = ?1 AND holder IS NULL",
      params![number.get(), holder_text(holder)?],
    )?;
    Ok(taken == 1)
  }

  fn release_workspace(&mut self, number: WorkspaceNumber, holder: Holder) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE workspaces SET holder = NULL WHERE number = ?1 AND holder = ?2",
      params![number.get(), holder_text(holder)?],
    )?;
    Ok(())
  }

  fn set_workspace_stranded(&mut self, number: WorkspaceNumber, stranded: Option<Stranded>) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO workspaces (number, stranded_run_id, stranded_files) VALUES (?1, ?2, ?3) \
       ON CONFLICT (number) DO UPDATE SET stranded_run_id = excluded.stranded_run_id, \
       stranded_files = excluded.stranded_files",
      params![
        number.get(),
        stranded.and_then(|stranded| stranded.run).map(|run| run.to_string()),
        stranded.map(|stranded| stranded.files),
      ],
    )?;
    Ok(())
  }

  fn set_workspace_cold(&mut self, number: WorkspaceNumber) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE workspaces SET warmth = 'cold' WHERE number = ?1",
      [number.get()],
    )?;
    Ok(())
  }

  fn end_workspace_wait(&mut self, run: RunId) -> Result<(), StoreError> {
    self
      .connection
      .execute("DELETE FROM workspace_waits WHERE run_id = ?1", [run.to_string()])?;
    Ok(())
  }

  fn set_run_borrows(&mut self, run: RunId, borrows: bool) -> Result<(), StoreError> {
    let sql = if borrows {
      "INSERT INTO borrowing_runs (run_id) VALUES (?1) ON CONFLICT (run_id) DO NOTHING"
    } else {
      "DELETE FROM borrowing_runs WHERE run_id = ?1"
    };
    self.connection.execute(sql, [run.to_string()])?;
    Ok(())
  }

  fn run_borrows(&self, run: RunId) -> Result<bool, StoreError> {
    Ok(
      self
        .connection
        .query_row(
          "SELECT 1 FROM borrowing_runs WHERE run_id = ?1",
          [run.to_string()],
          |_| Ok(()),
        )
        .optional()?
        .is_some(),
    )
  }

  fn workspace_waits(&self) -> Result<Vec<(RunId, Timestamp)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT run_id, since FROM workspace_waits ORDER BY since, run_id")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    rows
      .map(|row| {
        let (run, since) = row?;
        Ok((parse("run id", &run)?, Timestamp::from_unix_millis(since)))
      })
      .collect()
  }

  fn workspace_settings(&self) -> Result<WorkspaceSettings, StoreError> {
    let row: Option<(bool, Option<String>)> = self
      .connection
      .query_row("SELECT enabled, warm_up_command FROM workspace_settings", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })
      .optional()?;
    let (enabled, command) = row.unwrap_or_default();
    Ok(WorkspaceSettings {
      enabled,
      warm_up: command
        .map(|command| WarmUpCommand::new(&command).map_err(|error| corrupt("warm-up command", error)))
        .transpose()?,
      folders: warm_folders(&self.connection)?,
    })
  }

  fn save_workspace_settings(&mut self, settings: &WorkspaceSettings) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO workspace_settings (id, enabled, warm_up_command) VALUES (1, ?1, ?2) \
       ON CONFLICT (id) DO UPDATE SET enabled = excluded.enabled, warm_up_command = excluded.warm_up_command",
      params![settings.enabled, settings.warm_up.as_ref().map(WarmUpCommand::as_str)],
    )?;
    transaction.execute("DELETE FROM warm_folders", [])?;
    for (position, folder) in settings.folders.iter().enumerate() {
      transaction.execute(
        "INSERT INTO warm_folders (pattern, position, choice, last_seen_at) VALUES (?1, ?2, ?3, ?4)",
        params![
          folder.pattern.as_str(),
          i64::try_from(position).map_err(|_| StoreError::Unrepresentable("warm folder position"))?,
          choice_text(folder.choice),
          folder.last_seen.map(Timestamp::unix_millis),
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  fn warm_up_failure(&self) -> Result<Option<WarmUpFailure>, StoreError> {
    let row: Option<(u32, Option<i32>, i64, String)> = self
      .connection
      .query_row(
        "SELECT workspace, exit_code, failed_at, log FROM warm_up_failures",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    row
      .map(|(workspace, exit_code, failed_at, log)| {
        Ok(WarmUpFailure {
          workspace: workspace_number(workspace)?,
          exit_code,
          failed_at: Timestamp::from_unix_millis(failed_at),
          log,
        })
      })
      .transpose()
  }

  fn save_warm_up_failure(&mut self, failure: Option<&WarmUpFailure>) -> Result<(), StoreError> {
    match failure {
      Some(failure) => self.connection.execute(
        "INSERT INTO warm_up_failures (id, workspace, exit_code, failed_at, log) VALUES (1, ?1, ?2, ?3, ?4) \
         ON CONFLICT (id) DO UPDATE SET workspace = excluded.workspace, exit_code = excluded.exit_code, \
         failed_at = excluded.failed_at, log = excluded.log",
        params![
          failure.workspace.get(),
          failure.exit_code,
          failure.failed_at.unix_millis(),
          failure.log
        ],
      )?,
      None => self.connection.execute("DELETE FROM warm_up_failures", [])?,
    };
    Ok(())
  }
}

fn records(connection: &Connection) -> Result<Vec<WorkspaceRecord>, StoreError> {
  let mut statement = connection.prepare(&format!("SELECT {COLUMNS} ORDER BY number"))?;
  let rows = statement.query_map([], |row| Ok(StoredRecord::read(row)))?;
  rows.map(|row| row?.and_then(StoredRecord::record)).collect()
}

/// A `workspaces` row as SQLite gives it.
struct StoredRecord {
  number: u32,
  holder: Option<String>,
  last_used: Option<i64>,
  last_run: Option<String>,
  branch: Option<String>,
  warmth: String,
  stranded_run: Option<String>,
  stranded_files: Option<u32>,
}

impl StoredRecord {
  fn read(row: &Row<'_>) -> Result<Self, StoreError> {
    Ok(Self {
      number: row.get(0)?,
      holder: row.get(1)?,
      last_used: row.get(2)?,
      last_run: row.get(3)?,
      branch: row.get(4)?,
      warmth: row.get(5)?,
      stranded_run: row.get(6)?,
      stranded_files: row.get(7)?,
    })
  }

  fn record(self) -> Result<WorkspaceRecord, StoreError> {
    let stranded = self
      .stranded_files
      .map(|files| {
        Ok::<_, StoreError>(Stranded {
          run: self.stranded_run.map(|run| parse("run id", &run)).transpose()?,
          files,
        })
      })
      .transpose()?;
    Ok(WorkspaceRecord {
      number: workspace_number(self.number)?,
      holder: self
        .holder
        .map(|holder| serde_json::from_str(&holder).map_err(|error| corrupt("workspace holder", error)))
        .transpose()?,
      last_used: self.last_used.map(Timestamp::from_unix_millis),
      last_run: self.last_run.map(|run| parse("run id", &run)).transpose()?,
      branch: self
        .branch
        .map(|branch| BranchName::new(&branch).map_err(|error| corrupt("workspace branch", error)))
        .transpose()?,
      warmth: match self.warmth.as_str() {
        "cold" => Warmth::Cold,
        "warm" => Warmth::Warm,
        other => return Err(corrupt("workspace warmth", other)),
      },
      stranded,
    })
  }
}

fn warm_folders(connection: &Connection) -> Result<Vec<WarmFolder>, StoreError> {
  let mut statement = connection.prepare("SELECT pattern, choice, last_seen_at FROM warm_folders ORDER BY position")?;
  let rows = statement.query_map([], |row| {
    Ok((
      row.get::<_, String>(0)?,
      row.get::<_, String>(1)?,
      row.get::<_, Option<i64>>(2)?,
    ))
  })?;
  rows
    .map(|row| {
      let (pattern, choice, last_seen) = row?;
      Ok(WarmFolder {
        pattern: WarmPattern::new(&pattern).map_err(|error| corrupt("warm folder", error))?,
        choice: match choice.as_str() {
          "keep" => WarmChoice::Keep,
          "clean" => WarmChoice::Clean,
          other => return Err(corrupt("warm folder choice", other)),
        },
        last_seen: last_seen.map(Timestamp::from_unix_millis),
      })
    })
    .collect()
}

const fn choice_text(choice: WarmChoice) -> &'static str {
  match choice {
    WarmChoice::Keep => "keep",
    WarmChoice::Clean => "clean",
  }
}

fn holder_text(holder: Holder) -> Result<String, StoreError> {
  serde_json::to_string(&holder).map_err(|_| StoreError::Unrepresentable("workspace holder"))
}

fn workspace_number(number: u32) -> Result<WorkspaceNumber, StoreError> {
  WorkspaceNumber::new(number).ok_or_else(|| corrupt("workspace number", number))
}
