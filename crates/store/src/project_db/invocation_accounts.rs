//! The account each agent CLI invocation started on (`invocation_accounts`), written once by the runner before the
//! CLI starts; a run reads back which accounts its steps ran on.

use orchester_core::id::{AgentAccountId, InvocationId, RunId, StepExecutionId};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Records that `invocation` starts on `account`.
  pub fn save_invocation_account(
    &mut self,
    invocation: InvocationId,
    account: AgentAccountId,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO invocation_accounts (invocation_id, account_id) VALUES (?1, ?2)",
      params![invocation.to_string(), account.to_string()],
    )?;
    Ok(())
  }

  /// The account the latest agent CLI invocation of `execution` started on, if it recorded one.
  pub fn execution_account(&self, execution: StepExecutionId) -> Result<Option<AgentAccountId>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT accounts.account_id FROM invocation_accounts AS accounts \
         JOIN invocations ON invocations.id = accounts.invocation_id \
         WHERE invocations.step_execution_id = ?1 ORDER BY invocations.started_at DESC, invocations.id DESC LIMIT 1",
        [execution.to_string()],
        |row| row.get::<_, String>(0),
      )
      .optional()?
      .map(|account| parse("account id", &account))
      .transpose()
  }

  /// The account of each agent CLI invocation of `run` that recorded one, in the order they started.
  pub fn invocation_accounts(&self, run: RunId) -> Result<Vec<(InvocationId, AgentAccountId)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT accounts.invocation_id, accounts.account_id FROM invocation_accounts AS accounts \
       JOIN invocations ON invocations.id = accounts.invocation_id \
       WHERE invocations.run_id = ?1 ORDER BY invocations.started_at, invocations.id",
    )?;
    let rows = statement
      .query_map([run.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(invocation, account)| Ok((parse("invocation id", &invocation)?, parse("account id", &account)?)))
      .collect()
  }
}
