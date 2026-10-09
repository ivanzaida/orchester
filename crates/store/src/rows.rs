//! Conversions between stored columns and domain values. Reads re-validate through the domain
//! constructors, so a damaged row surfaces as `StoreError::Corrupt` instead of a bad value.

use std::{fmt::Display, path::Path, str::FromStr, time::Duration};

use orchester_core::tokens::TokenBudget;
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

/// A token count or budget as an `INTEGER` column.
pub(crate) fn tokens_column(what: &'static str, tokens: u64) -> Result<i64, StoreError> {
  i64::try_from(tokens).map_err(|_| StoreError::Unrepresentable(what))
}

pub(crate) fn budget_value(what: &'static str, tokens: i64) -> Result<TokenBudget, StoreError> {
  u64::try_from(tokens)
    .ok()
    .and_then(|tokens| TokenBudget::new(tokens).ok())
    .ok_or_else(|| corrupt(what, tokens))
}

/// A duration as whole milliseconds in an `INTEGER` column.
pub(crate) fn millis_column(what: &'static str, duration: Duration) -> Result<i64, StoreError> {
  i64::try_from(duration.as_millis()).map_err(|_| StoreError::Unrepresentable(what))
}

pub(crate) fn duration_value(what: &'static str, millis: i64) -> Result<Duration, StoreError> {
  u64::try_from(millis)
    .map(Duration::from_millis)
    .map_err(|_| corrupt(what, millis))
}

/// A budget that may be no limit, as a nullable `INTEGER` column: `NULL` is no limit.
pub(crate) fn optional_budget_column(
  what: &'static str,
  budget: Option<TokenBudget>,
) -> Result<Option<i64>, StoreError> {
  budget.map(|budget| tokens_column(what, budget.get())).transpose()
}

pub(crate) fn optional_budget_value(
  what: &'static str,
  tokens: Option<i64>,
) -> Result<Option<TokenBudget>, StoreError> {
  tokens.map(|tokens| budget_value(what, tokens)).transpose()
}

/// A time limit that may be no limit, as whole milliseconds in a nullable `INTEGER` column.
pub(crate) fn optional_millis_column(what: &'static str, limit: Option<Duration>) -> Result<Option<i64>, StoreError> {
  limit.map(|limit| millis_column(what, limit)).transpose()
}

pub(crate) fn optional_duration_value(what: &'static str, millis: Option<i64>) -> Result<Option<Duration>, StoreError> {
  millis.map(|millis| duration_value(what, millis)).transpose()
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
  let (found, body) = decode_document(what, text)?;
  if found != schema {
    return Err(corrupt(
      what,
      format!("schema {found} is not the supported schema {schema}"),
    ));
  }
  decode_body(what, body)
}

/// The schema and the body of a document written by [`encode_json`], for values whose earlier
/// schemas are upgraded when read.
pub(crate) fn decode_document(what: &'static str, text: &str) -> Result<(u32, Value), StoreError> {
  let mut document: Value = serde_json::from_str(text).map_err(|error| corrupt(what, error))?;
  let schema = document
    .get("schema")
    .and_then(Value::as_u64)
    .and_then(|schema| u32::try_from(schema).ok())
    .ok_or_else(|| corrupt(what, "the schema is missing"))?;
  let body = document
    .get_mut("body")
    .map(Value::take)
    .ok_or_else(|| corrupt(what, "the body is missing"))?;
  Ok((schema, body))
}

/// A body read by [`decode_document`], through the domain constructors.
pub(crate) fn decode_body<T: DeserializeOwned>(what: &'static str, body: Value) -> Result<T, StoreError> {
  serde_json::from_value(body).map_err(|error| corrupt(what, error))
}
