//! The parts of an `http://` or `https://` URL that connections check, read the same way for an MCP
//! server's URL ([`super::mcp::McpUrl`]) and a native connection's base URL ([`super::BaseUrl`]).

/// The scheme of an HTTP URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scheme {
  Http,
  Https,
}

/// The scheme of `url`, matched without regard to case, and the rest of the URL after it; `None`
/// for any other scheme.
pub(super) fn split_scheme(url: &str) -> Option<(Scheme, &str)> {
  [("http://", Scheme::Http), ("https://", Scheme::Https)]
    .into_iter()
    .find_map(|(prefix, scheme)| {
      let head = url.get(..prefix.len())?;
      head
        .eq_ignore_ascii_case(prefix)
        .then(|| (scheme, &url[prefix.len()..]))
    })
}

/// The authority at the start of `rest`, a URL after its scheme: everything before the first `/`,
/// `?` or `#`.
pub(super) fn authority(rest: &str) -> &str {
  rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// The authority and path of `rest`, a URL after its scheme: everything before the first `?` or `#`.
pub(super) fn before_query(rest: &str) -> &str {
  rest.split(['?', '#']).next().unwrap_or(rest)
}

/// HTTP clients read a backslash before the query as a slash, so there it could end the host where
/// these checks do not.
pub(super) const BACKSLASH: char = '\\';

/// Code points a domain cannot have (the URL Standard's forbidden domain code points, less the
/// controls and spaces refused before). Any other character, a combining mark of an IDN label
/// included, may be part of a host name.
pub(super) const FORBIDDEN_IN_DOMAIN: [char; 13] = ['#', '%', '/', ':', '<', '>', '?', '@', '[', '\\', ']', '^', '|'];

/// Whether a bracketed IPv6 address may name a zone after `%25` (RFC 6874, `[fe80::1%25eth0]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ZoneId {
  Allowed,
  Refused,
}

/// Why an authority names no usable host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostError {
  /// Nothing before the port.
  Missing,
  /// A bracketed IPv6 address that does not close, holds something other than an address (and a
  /// zone id where allowed), or is followed by something other than `:`.
  Invalid,
}

/// `authority` split into its host and the text after the `:` that ends the host, if any. A
/// bracketed IPv6 address keeps its brackets and is checked; a host name and the port are left to
/// the caller.
pub(super) fn split_host(authority: &str, zone: ZoneId) -> Result<(&str, Option<&str>), HostError> {
  let Some(inside) = authority.strip_prefix('[') else {
    let (host, port) = match authority.split_once(':') {
      Some((host, port)) => (host, Some(port)),
      None => (authority, None),
    };
    return if host.is_empty() {
      Err(HostError::Missing)
    } else {
      Ok((host, port))
    };
  };
  let end = inside.find(']').ok_or(HostError::Invalid)?;
  if !is_address(&inside[..end], zone) {
    return Err(HostError::Invalid);
  }
  let port = match &inside[end + 1..] {
    "" => None,
    after => Some(after.strip_prefix(':').ok_or(HostError::Invalid)?),
  };
  Ok((&authority[..end + 2], port))
}

/// Whether `text`, the inside of brackets, is an IPv6 address, then a zone id where `zone` allows.
fn is_address(text: &str, zone: ZoneId) -> bool {
  let (address, zone_id) = match (zone, text.split_once("%25")) {
    (ZoneId::Allowed, Some((address, zone_id))) => (address, Some(zone_id)),
    _ => (text, None),
  };
  let address_char = |c: char| c.is_ascii_hexdigit() || c == ':' || c == '.';
  // A zone id is unreserved characters and percent-encodings.
  let zone_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~' | '%');
  !address.is_empty()
    && address.chars().all(address_char)
    && zone_id.is_none_or(|zone_id| !zone_id.is_empty() && zone_id.chars().all(zone_char))
}
