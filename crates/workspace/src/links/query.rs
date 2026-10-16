//! A link's query: every value percent-encoded, so a value with spaces, `&`, `?` or `#` stays one value.

use std::fmt::Write as _;

/// `path` with a query of the `params` that have a value, in their order.
pub fn with_query(mut path: String, params: &[(&str, Option<&str>)]) -> String {
  let mut separator = '?';
  for (name, value) in params {
    if let Some(value) = value {
      write!(path, "{separator}{name}={}", encode(value)).expect("writing to a String cannot fail");
      separator = '&';
    }
  }
  path
}

/// A file's path as a route's segments: each part percent-encoded with its `/` kept as the path's own, except a
/// `/` that would leave an empty segment (a leading one, as in an absolute path, or a doubled one), which is
/// encoded, so any path a link names reads back, even one the code viewer refuses.
pub fn encode_path(path: &str) -> String {
  let parts: Vec<&str> = path.split('/').collect();
  let mut encoded = String::with_capacity(path.len());
  for (at, part) in parts.iter().enumerate() {
    if at > 0 {
      let empty_around = parts[at - 1].is_empty() || part.is_empty();
      encoded.push_str(if empty_around { "%2F" } else { "/" });
    }
    encoded.push_str(&encode(part));
  }
  encoded
}

/// `value` with every byte outside the unreserved characters and `/` percent-encoded, as a route's query
/// value is written.
pub fn encode(value: &str) -> String {
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
