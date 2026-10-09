//! The query of a route's path (`?tab=needs_you&q=retry%20jitter`). Every value is percent-encoded, so a
//! search with spaces or a file path with `&`, `?` or `#` stays one value and reads back as it was.

use std::fmt::Write as _;

use super::RouteError;

/// The decoded parameters of a path's query. A route's parser takes the ones its screen takes; one left
/// over is a parameter the screen does not take.
pub(super) struct Params(Vec<(String, String)>);

impl Params {
  pub(super) fn parse(query: &str) -> Result<Self, RouteError> {
    let params = query
      .split('&')
      .filter(|pair| !pair.is_empty())
      .map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let decoded = decode(value).ok_or_else(|| RouteError::InvalidParam {
          param: name.to_owned(),
          value: value.to_owned(),
        })?;
        Ok((name.to_owned(), decoded))
      })
      .collect::<Result<_, RouteError>>()?;
    Ok(Self(params))
  }

  /// The value of `name`, taken out of the query; a repeated parameter stays for [`Params::finish`].
  pub(super) fn take(&mut self, name: &str) -> Option<String> {
    let index = self.0.iter().position(|(param, _)| param == name)?;
    Some(self.0.remove(index).1)
  }

  /// The value of `name` that `read` understands, or the default when the query has none.
  pub(super) fn take_or_default<T: Default>(
    &mut self,
    name: &str,
    read: impl FnOnce(&str) -> Option<T>,
  ) -> Result<T, RouteError> {
    let Some(value) = self.take(name) else {
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

/// `path` with a query of the `params` that have a value, in their order.
pub(super) fn with_query(mut path: String, params: &[(&str, Option<&str>)]) -> String {
  let mut separator = '?';
  for (name, value) in params {
    if let Some(value) = value {
      write!(path, "{separator}{name}={}", encode(value)).expect("writing to a String cannot fail");
      separator = '&';
    }
  }
  path
}

/// `value` with every byte outside the unreserved characters and `/` percent-encoded.
fn encode(value: &str) -> String {
  let mut encoded = String::with_capacity(value.len());
  for byte in value.bytes() {
    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
      encoded.push(char::from(byte));
    } else {
      write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
    }
  }
  encoded
}

/// [`encode`] undone; `None` for a malformed escape or bytes that are not UTF-8.
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
