//! What each agent CLI launch was told (`invocation_prompts`): its injection event and the source
//! of every part of its prompt, written by the runner before the CLI starts.

use orchester_core::{id::InvocationId, run::InvocationPrompt};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
  StoreError,
  rows::{decode_json, encode_json},
};

const SCHEMA: u32 = 1;
const WHAT: &str = "invocation prompt";

pub(super) fn save(
  connection: &Connection,
  invocation: InvocationId,
  prompt: &InvocationPrompt,
) -> Result<(), StoreError> {
  let changed = connection.execute(
    "INSERT INTO invocation_prompts (invocation_id, body) SELECT ?1, ?2 WHERE EXISTS \
     (SELECT 1 FROM invocations WHERE id = ?1)",
    params![invocation.to_string(), encode_json(WHAT, SCHEMA, prompt)?],
  )?;
  if changed == 0 {
    return Err(StoreError::InvocationNotFound(invocation));
  }
  Ok(())
}

pub(super) fn of_invocation(
  connection: &Connection,
  invocation: InvocationId,
) -> Result<Option<InvocationPrompt>, StoreError> {
  let body: Option<String> = connection
    .query_row(
      "SELECT body FROM invocation_prompts WHERE invocation_id = ?1",
      [invocation.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  body.map(|body| decode_json(WHAT, SCHEMA, &body)).transpose()
}
