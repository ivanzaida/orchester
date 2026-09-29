//! Import from repository (ORC-15 `wulRC`) is a dialog over the page it was opened from, like
//! Import resources ([`super::import`]): the page's path carries it as the `from-repository` query,
//! so going back closes it and the page under it stays as it was.

use super::import::without_query;

const QUERY: &str = "from-repository";

/// `path` with Import from repository open over it.
pub fn open(path: &str) -> String {
  let under = close(path);
  let separator = if under.contains('?') { '&' } else { '?' };
  format!("{under}{separator}{QUERY}")
}

/// Whether Import from repository is open over `path`.
pub fn shown(path: &str) -> bool {
  path
    .split_once('?')
    .is_some_and(|(_, query)| query.split('&').any(|pair| pair.split('=').next() == Some(QUERY)))
}

/// `path` without Import from repository over it.
pub fn close(path: &str) -> String {
  without_query(path, QUERY)
}
