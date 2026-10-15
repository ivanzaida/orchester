//! The Usage page's path and query: `/project/<id>/usage`, with the period, grouping and filters
//! `orchester_workspace::usage::UsageQuery` reads and writes as its parameters, so a link `get_usage`
//! returns opens the page as it was asked for.

use orchester_workspace::usage::{ParamsError, QueryError, SEGMENT, UsageQuery};

use super::{RouteError, query::Params};

pub(super) fn path(project_root: &str, query: &UsageQuery) -> String {
  format!("{project_root}/{SEGMENT}{}", query.query_string())
}

pub(super) fn take(params: &mut Params) -> Result<UsageQuery, RouteError> {
  UsageQuery::from_params(|name| params.take(name)).map_err(|error| match error {
    ParamsError::Query(QueryError { param, value }) => RouteError::InvalidParam {
      param: param.to_owned(),
      value,
    },
    ParamsError::Source(error) => error,
  })
}
