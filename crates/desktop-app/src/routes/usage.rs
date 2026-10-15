//! The Usage page's path and query: `/project/<id>/usage`, and every project's `/usage`, with the period,
//! grouping and filters `orchester_workspace::usage::UsageQuery` reads and writes as its parameters, written
//! as `orchester_workspace::links` writes the Usage links, so a link `get_usage` returns opens the page as it
//! was asked for. Only every project's page chooses projects.

pub(super) use orchester_workspace::links::USAGE as SEGMENT;
use orchester_workspace::{
  links::with_usage_query,
  usage::{ParamsError, QueryError, UsageQuery},
};

use super::{RouteError, query::Params};

pub(super) fn path(project_root: &str, query: &UsageQuery) -> String {
  with_usage_query(format!("{project_root}/{SEGMENT}"), query)
}

pub(super) fn everywhere_path(query: &UsageQuery) -> String {
  with_usage_query(format!("/{SEGMENT}"), query)
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
