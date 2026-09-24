//! Conversions between stored columns and domain values. Reads re-validate through the domain
//! constructors, so a damaged row surfaces as `StoreError::Corrupt` instead of a bad value.

use std::{fmt::Display, path::Path, str::FromStr};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::StoreError;

pub(crate) fn parse<T>(what: &'static str, value: &str) -> Result<T, StoreError>
where
  T: FromStr,
  T::Err: Display,
{
  value.parse().map_err(|error: T::Err| corrupt(what, error))
}

pub(crate) fn corrupt(what: &'static str, detail: impl Display) -> StoreError {
  StoreError::Corrupt {
    what,
    detail: detail.to_string(),
  }
}

pub(crate) fn path_text<'a>(what: &'static str, path: &'a Path) -> Result<&'a str, StoreError> {
  path.to_str().ok_or(StoreError::Unrepresentable(what))
}

/// Stores a JSON value as `{"schema": n, "body": …}` so its shape can evolve without a table
/// migration.
pub(crate) fn encode_json<T: Serialize>(what: &'static str, schema: u32, body: &T) -> Result<String, StoreError> {
  let body = serde_json::to_value(body).map_err(|_| StoreError::Unrepresentable(what))?;
  Ok(json!({ "schema": schema, "body": body }).to_string())
}

/// Reads a value written by [`encode_json`] back through the domain constructors.
pub(crate) fn decode_json<T: DeserializeOwned>(what: &'static str, schema: u32, text: &str) -> Result<T, StoreError> {
  let mut document: Value = serde_json::from_str(text).map_err(|error| corrupt(what, error))?;
  let found = document.get("schema").and_then(Value::as_u64);
  if found != Some(u64::from(schema)) {
    return Err(corrupt(
      what,
      format!("schema {found:?} is not the supported schema {schema}"),
    ));
  }
  let body = document
    .get_mut("body")
    .map(Value::take)
    .ok_or_else(|| corrupt(what, "the body is missing"))?;
  serde_json::from_value(body).map_err(|error| corrupt(what, error))
}
