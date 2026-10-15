//! A project's usage as the Usage page (ORC-15, `et35n` in `data/orchester2.pen`) and `get_usage` read it:
//! tokens and active agent time over a range of local days, against as many days before, filtered and
//! grouped as a query says ([`UsageQuery`], which a link carries, [`usage_link`]). What counts and how is
//! `orchester_core::usage`; this module reads it ([`UsageData`]), works out what the page shows
//! ([`Dashboard`]) and what its filters offer and choose ([`Facet`]).

mod dashboard;
mod filters;
mod link;
mod load;
mod names;
mod query;

pub use self::{
  dashboard::{Changes, Dashboard, DashboardState},
  filters::{Facet, FilterOption, FilterValue, ValueLabel},
  link::{SEGMENT, usage_link},
  load::{Scope, UsageData},
  query::{
    CustomRangeError, DAY_FORMAT, ParamsError, QueryError, UsageQuery, cli_name, cli_of, custom_range, group_name,
    group_of,
  },
};
