//! The Usage page's links: a project's `/project/<id>/usage` and every project's `/usage` (All projects'
//! *Usage* tab), with the period, grouping and filters [`UsageQuery::params`] writes, each value encoded as
//! every link's query is. The app's router writes the page's path with [`with_usage_query`] too.

use super::with_query;
use crate::usage::UsageQuery;

/// The page's segment, under a project's path or alone.
pub const SEGMENT: &str = "usage";

/// `path` with the parameters of `query` that differ from the defaults, in their order.
pub fn with_usage_query(path: String, query: &UsageQuery) -> String {
  let params = query.params();
  let pairs: Vec<(&str, Option<&str>)> = params
    .iter()
    .map(|(name, value)| (*name, Some(value.as_str())))
    .collect();
  with_query(path, &pairs)
}
