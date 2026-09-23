//! Conversions between stored columns and domain values. Reads re-validate through the domain
//! constructors, so a damaged row surfaces as `StoreError::Corrupt` instead of a bad value.

use std::{fmt::Display, path::Path, str::FromStr};

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
