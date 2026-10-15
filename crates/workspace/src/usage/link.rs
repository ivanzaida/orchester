//! The app link of a project's Usage page with a query (`docs/design/orchestrator-toolset.md`, *Links*):
//! `/project/<project-id>/usage?from=2026-10-01&to=2026-10-15&group=task&agent=<profile-id>`. The app's
//! router writes the page's path with [`UsageQuery::query_string`] too, so the two never drift.

use std::fmt::Write as _;

use orchester_core::id::ProjectId;

use super::UsageQuery;

/// The page's path under a project's.
pub const SEGMENT: &str = "usage";

/// The Usage page of `project` showing `query`.
pub fn usage_link(project: ProjectId, query: &UsageQuery) -> String {
  format!("/project/{project}/{SEGMENT}{}", query.query_string())
}

impl UsageQuery {
  /// `?` and the parameters that differ from the defaults, each value percent-encoded; empty for the
  /// defaults.
  pub fn query_string(&self) -> String {
    let mut query = String::new();
    for (index, (name, value)) in self.params().iter().enumerate() {
      let separator = if index == 0 { '?' } else { '&' };
      write!(query, "{separator}{name}={}", encode(value)).expect("writing to a String cannot fail");
    }
    query
  }
}

/// `value` with every byte outside the unreserved characters and `/` percent-encoded, as the router
/// reads a query.
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
