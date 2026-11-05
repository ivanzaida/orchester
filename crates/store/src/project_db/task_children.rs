//! What a task keeps beside its row: its acceptance criteria, its organization and its story points. A listing of
//! tasks reads them for every task it lists at once, one statement per table, the tasks' ids bound as one JSON array.

use std::collections::HashMap;

use orchester_core::task::{estimate::StoryEstimate, organization::TaskOrganization};
use rusqlite::{Connection, Row};

use super::estimates::{self, EstimateRow};
use crate::{
  StoreError,
  rows::{corrupt, decode_json},
};

/// The criteria, organizations and estimates of the tasks a listing lists, by task id.
pub(super) struct TaskChildren {
  criteria: HashMap<String, Vec<String>>,
  organizations: HashMap<String, String>,
  estimates: HashMap<String, EstimateRow>,
}

impl TaskChildren {
  /// Of the tasks of `ids`.
  pub(super) fn read<'a>(connection: &Connection, ids: impl Iterator<Item = &'a str>) -> Result<Self, StoreError> {
    let ids = serde_json::to_string(&ids.collect::<Vec<_>>()).map_err(|error| corrupt("task ids", error))?;
    let mut children = Self {
      criteria: HashMap::new(),
      organizations: HashMap::new(),
      estimates: HashMap::new(),
    };
    let listed = "task_id IN (SELECT value FROM json_each(?1))";
    for (task, text) in rows(
      connection,
      &format!("SELECT task_id, text FROM task_criteria WHERE {listed} ORDER BY position"),
      &ids,
      |row| row.get::<_, String>(1),
    )? {
      children.criteria.entry(task).or_default().push(text);
    }
    children.organizations = rows(
      connection,
      &format!("SELECT task_id, body FROM task_organization WHERE {listed}"),
      &ids,
      |row| row.get::<_, String>(1),
    )?
    .into_iter()
    .collect();
    children.estimates = rows(
      connection,
      &format!("SELECT task_id, points, source, reason, basis, set_at FROM task_estimates WHERE {listed}"),
      &ids,
      |row| Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    )?
    .into_iter()
    .collect();
    Ok(children)
  }

  /// The criteria of task `id`, in order.
  pub(super) fn criteria(&mut self, id: &str) -> Vec<String> {
    self.criteria.remove(id).unwrap_or_default()
  }

  /// The organization of task `id`; the default when it has none stored.
  pub(super) fn organization(&mut self, id: &str) -> Result<TaskOrganization, StoreError> {
    self.organizations.remove(id).map_or_else(
      || Ok(TaskOrganization::default()),
      |body| decode_json("task organization", 1, &body),
    )
  }

  /// The story points of task `id`, once it has some.
  pub(super) fn estimate(&mut self, id: &str) -> Result<Option<StoryEstimate>, StoreError> {
    self.estimates.remove(id).map(estimates::restored).transpose()
  }
}

/// The rows `query` returns with the ids bound, each with the task id in its first column and the rest as `columns`
/// reads it.
fn rows<T>(
  connection: &Connection,
  query: &str,
  ids: &str,
  columns: impl Fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<(String, T)>, StoreError> {
  let mut statement = connection.prepare(query)?;
  let rows = statement
    .query_map([ids], |row| Ok((row.get::<_, String>(0)?, columns(row)?)))?
    .collect::<Result<Vec<_>, _>>()?;
  Ok(rows)
}
