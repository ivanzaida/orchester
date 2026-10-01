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
