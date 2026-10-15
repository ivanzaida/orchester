//! The Usage page's path and query: `/project/<id>/usage`, and every project's `/usage`, with the period,
//! grouping and filters `orchester_workspace::usage::UsageQuery` reads and writes as its parameters, so a
//! link `get_usage` returns opens the page as it was asked for. Only every project's page chooses projects.

pub(super) use orchester_workspace::usage::SEGMENT;
use orchester_workspace::usage::{ParamsError, QueryError, UsageQuery};

use super::{RouteError, query::Params};

pub(super) fn path(project_root: &str, query: &UsageQuery) -> String {
  format!("{project_root}/{SEGMENT}{}", query.query_string())
}

pub(super) fn everywhere_path(query: &UsageQuery) -> String {
  format!("/{SEGMENT}{}", query.query_string())
}

/// A project's page's query; a `project` parameter is refused.
pub(super) fn take(params: &mut Params) -> Result<UsageQuery, RouteError> {
  let query = take_everywhere(params)?;
  match query.filter.projects.first() {
    Some(project) => Err(RouteError::InvalidParam {
      param: "project".to_owned(),
      value: project.to_string(),
    }),
    None => Ok(query),
  }
}

pub(super) fn take_everywhere(params: &mut Params) -> Result<UsageQuery, RouteError> {
  UsageQuery::from_params(|name| params.take(name)).map_err(|error| match error {
    ParamsError::Query(QueryError { param, value }) => RouteError::InvalidParam {
      param: param.to_owned(),
      value,
    },
    ParamsError::Source(error) => error,
  })
}
