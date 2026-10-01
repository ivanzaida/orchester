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

/// HTTP clients read a backslash as a slash, so in a URL it could end the host where these checks
/// do not; a URL with one is refused.
pub(super) const BACKSLASH: char = '\\';

/// Why an authority names no usable host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostError {
  /// Nothing before the port.
  Missing,
  /// Characters a host name cannot have, an unclosed or empty IPv6 address, or a port that is not
  /// a number.
  Invalid,
}

/// The host of `authority`, checked with its port: a bracketed IPv6 address or a name of letters,
/// digits, `-`, `.` and `_`, then nothing or `:` and a port number.
pub(super) fn host(authority: &str) -> Result<&str, HostError> {
  let (host, port) = match authority.strip_prefix('[') {
    Some(inside) => {
      let end = inside.find(']').ok_or(HostError::Invalid)?;
      let address = &inside[..end];
      if address.is_empty() || !address.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.') {
        return Err(HostError::Invalid);
      }
      let port = match &inside[end + 1..] {
        "" => None,
        after => Some(after.strip_prefix(':').ok_or(HostError::Invalid)?),
      };
      (&authority[..end + 2], port)
    }
    None => match authority.split_once(':') {
      Some((host, port)) => (host, Some(port)),
      None => (authority, None),
    },
  };
  if host.is_empty() {
    return Err(HostError::Missing);
  }
  let name = |c: char| c.is_alphanumeric() || matches!(c, '-' | '.' | '_');
  if !host.starts_with('[') && !host.chars().all(name) {
    return Err(HostError::Invalid);
  }
  // `u16` parsing alone would take a sign.
  let valid_port = |port: &str| port.chars().all(|c| c.is_ascii_digit()) && port.parse::<u16>().is_ok();
  if port.is_some_and(|port| !valid_port(port)) {
    return Err(HostError::Invalid);
  }
  Ok(host)
}
