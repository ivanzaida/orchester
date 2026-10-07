//! The code index (ORC-38): `project_code_index`, Project settings › Code index, one row, written
//! only when a person sets it; a project without the row has it off (ORC-69 made off the default; a
//! project turned on before keeps its `on` row, so nobody's choice changes). `invocation_code_index`
//! and `invocation_edit_progress`, what each agent invocation got of the index and used before its
//! first edit.

use orchester_core::{
  code_index::{CodeIndexSetting, CodeIndexUse},
  id::InvocationId,
  run::EditProgress,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;

use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json},
};

const SCHEMA: u32 = 1;
const CODE_INDEX_USE: &str = "invocation code index";
const EDIT_PROGRESS: &str = "invocation edit progress";

pub(super) fn load_setting(connection: &Connection) -> Result<CodeIndexSetting, StoreError> {
  let setting: Option<String> = connection
    .query_row("SELECT setting FROM project_code_index", [], |row| row.get(0))
    .optional()?;
  match setting.as_deref() {
    Some("on") => Ok(CodeIndexSetting::On),
    None | Some("off") => Ok(CodeIndexSetting::Off),
    Some(other) => Err(corrupt("code index setting", other)),
  }
}

pub(super) fn save_setting(connection: &Connection, setting: CodeIndexSetting) -> Result<(), StoreError> {
  let text = match setting {
    CodeIndexSetting::On => "on",
    CodeIndexSetting::Off => "off",
  };
  connection.execute(
    "INSERT INTO project_code_index (id, setting) VALUES (1, ?1) \
     ON CONFLICT (id) DO UPDATE SET setting = excluded.setting",
    params![text],
  )?;
  Ok(())
}

pub(super) fn save_use(
  connection: &Connection,
  invocation: InvocationId,
  used: &CodeIndexUse,
) -> Result<(), StoreError> {
  let changed = connection.execute(
    "INSERT INTO invocation_code_index (invocation_id, body) SELECT ?1, ?2 WHERE EXISTS \
     (SELECT 1 FROM invocations WHERE id = ?1)",
    params![invocation.to_string(), encode_json(CODE_INDEX_USE, SCHEMA, used)?],
  )?;
  if changed == 0 {
    return Err(StoreError::InvocationNotFound(invocation));
  }
  Ok(())
}

pub(super) fn load_use(connection: &Connection, invocation: InvocationId) -> Result<Option<CodeIndexUse>, StoreError> {
  load(connection, "invocation_code_index", CODE_INDEX_USE, invocation)
}

pub(super) fn save_progress(
  connection: &Connection,
  invocation: InvocationId,
  progress: &EditProgress,
) -> Result<(), StoreError> {
  let changed = connection.execute(
    "INSERT INTO invocation_edit_progress (invocation_id, body) SELECT ?1, ?2 WHERE EXISTS \
     (SELECT 1 FROM invocations WHERE id = ?1) \
     ON CONFLICT (invocation_id) DO UPDATE SET body = excluded.body",
    params![invocation.to_string(), encode_json(EDIT_PROGRESS, SCHEMA, progress)?],
  )?;
  if changed == 0 {
    return Err(StoreError::InvocationNotFound(invocation));
  }
  Ok(())
}

pub(super) fn load_progress(
  connection: &Connection,
  invocation: InvocationId,
) -> Result<Option<EditProgress>, StoreError> {
  load(connection, "invocation_edit_progress", EDIT_PROGRESS, invocation)
}

/// The body stored for `invocation` in `table`, one of this module's tables.
fn load<T: DeserializeOwned>(
  connection: &Connection,
  table: &str,
  what: &'static str,
  invocation: InvocationId,
) -> Result<Option<T>, StoreError> {
  let body: Option<String> = connection
    .query_row(
      &format!("SELECT body FROM {table} WHERE invocation_id = ?1"),
      [invocation.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  body.map(|body| decode_json(what, SCHEMA, &body)).transpose()
}
