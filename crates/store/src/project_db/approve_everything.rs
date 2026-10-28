//! *With auto mode on, approve everything in each run* (`orchester_core::run::ApproveEverything`): off until someone
//! turns it on, and who did. The runner reads it at each request of a run that uses auto mode.

use orchester_core::run::{Answerer, ApproveEverything};
use rusqlite::OptionalExtension;

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  pub fn approve_everything(&self) -> Result<ApproveEverything, StoreError> {
    let by: Option<String> = self
      .connection
      .query_row(
        "SELECT turned_on_by FROM project_approve_everything WHERE id = 1",
        [],
        |row| row.get(0),
      )
      .optional()?;
    match by.as_deref() {
      None => Ok(ApproveEverything::OFF),
      Some("person") => Ok(ApproveEverything::on(Answerer::Person)),
      Some("automation") => Ok(ApproveEverything::on(Answerer::Automation)),
      Some(other) => Err(corrupt("approve everything setting", other)),
    }
  }

  pub fn save_approve_everything(&mut self, setting: ApproveEverything) -> Result<(), StoreError> {
    match setting.turned_on_by() {
      None => self
        .connection
        .execute("DELETE FROM project_approve_everything WHERE id = 1", [])?,
      Some(by) => self.connection.execute(
        "INSERT INTO project_approve_everything (id, turned_on_by) VALUES (1, ?1) \
         ON CONFLICT (id) DO UPDATE SET turned_on_by = excluded.turned_on_by",
        [match by {
          Answerer::Person => "person",
          Answerer::Automation => "automation",
        }],
      )?,
    };
    Ok(())
  }
}
