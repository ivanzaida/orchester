//! The query of a route's path (`?tab=needs_you&q=retry%20jitter`). Every value is percent-encoded, so a
//! search with spaces or a file path with `&`, `?` or `#` stays one value and reads back as it was.

pub(super) use orchester_workspace::links::with_query;

use super::RouteError;

/// The parameters of a path's query. A route's parser takes the ones its screen takes, once the path has
/// named the screen, and a value is decoded only then; one left over is a parameter the screen does not
/// take.
pub(super) struct Params(Vec<(String, String)>);

impl Params {
  /// The parameters of `query`, their values still encoded.
  pub(super) fn new(query: &str) -> Self {
    let params = query
      .split('&')
      .filter(|pair| !pair.is_empty())
      .map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (name.to_owned(), value.to_owned())
      })
      .collect();
    Self(params)
  }

  /// The decoded value of `name`, taken out of the query; a repeated parameter stays for
  /// [`Params::finish`].
  pub(super) fn take(&mut self, name: &str) -> Result<Option<String>, RouteError> {
    let Some(index) = self.0.iter().position(|(param, _)| param == name) else {
      return Ok(None);
    };
    let (param, value) = self.0.remove(index);
    match decode(&value) {
      Some(decoded) => Ok(Some(decoded)),
      None => Err(RouteError::InvalidParam { param, value }),
    }
  }

  /// The value of `name` that `read` understands, or the default when the query has none.
  pub(super) fn take_or_default<T: Default>(
    &mut self,
    name: &str,
    read: impl FnOnce(&str) -> Option<T>,
  ) -> Result<T, RouteError> {
    let Some(value) = self.take(name)? else {
      return Ok(T::default());
    };
    read(&value).ok_or_else(|| RouteError::InvalidParam {
      param: name.to_owned(),
      value,
    })
  }

  /// Fails on the first parameter no parser took.
  pub(super) fn finish(self) -> Result<(), RouteError> {
    match self.0.into_iter().next() {
      Some((param, _)) => Err(RouteError::UnexpectedParam { param }),
      None => Ok(()),
    }
  }
}

/// [`orchester_workspace::links::encode`] undone; `None` for a malformed escape or bytes that are not UTF-8.
fn decode(value: &str) -> Option<String> {
  let mut bytes = Vec::with_capacity(value.len());
  let mut rest = value.bytes();
  while let Some(byte) = rest.next() {
    if byte != b'%' {
      bytes.push(byte);
      continue;
    }
    let digits = [rest.next()?, rest.next()?];
    if !digits.iter().all(u8::is_ascii_hexdigit) {
      return None;
    }
    let hex = std::str::from_utf8(&digits).ok()?;
    bytes.push(u8::from_str_radix(hex, 16).ok()?);
  }
  String::from_utf8(bytes).ok()
}
