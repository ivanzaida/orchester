//! The Usage pages' routes, which carry their whole query as the workspace links it.

use chrono::NaiveDate;
use orchester_app::routes::{ProjectPage, Route, RouteError, Screen, routes};
use orchester_core::usage::{DayRange, GroupBy, Period};
use orchester_workspace::{links::Link, usage::UsageQuery};

use super::{AWKWARD, PROJECT, at, project};

/// The Usage page over a custom range by agent, filtered to a model with a space and an ampersand.
pub(super) fn usage_query() -> UsageQuery {
  let range = DayRange::new(
    NaiveDate::from_ymd_opt(2026, 10, 1).expect("a day"),
    NaiveDate::from_ymd_opt(2026, 10, 15).expect("a day"),
  )
  .expect("a range");
  let mut query = UsageQuery {
    period: Period::Custom(range),
    group: GroupBy::Agent,
    ..UsageQuery::default()
  };
  query.filter.models.push(AWKWARD.into());
  query.filter.tasks.push("KON-12".into());
  query
}

#[test]
fn usage_pages_carry_their_query_as_the_workspace_links_it() {
  let query = usage_query();
  let route = at(ProjectPage::Usage(Box::new(query.clone())));
  assert_eq!(
    route.path(),
    Link::Usage(project(), Box::new(query)).path(),
    "the router writes the link the tools return"
  );
  assert_eq!(Route::parse(&route.path()), Ok(route.clone()));
  assert_eq!(route.nav_screen(), Screen::Usage);
  assert_eq!(
    at(ProjectPage::Usage(Box::default())).path(),
    format!("/project/{PROJECT}/usage")
  );
  let table = routes();
  let chain = table.resolve(&route.path());
  assert_eq!(
    chain.last().expect("the usage route resolves").pattern_raw(),
    route.pattern()
  );
  for (query, param) in [
    ("period=week", "period"),
    ("group=month", "group"),
    ("from=2026-10-01", "from"),
    ("from=2025-01-01&to=2026-01-02", "to"),
    ("model=", "model"),
  ] {
    assert!(
      matches!(
        Route::parse(&format!("/project/{PROJECT}/usage?{query}")),
        Err(RouteError::InvalidParam { param: given, .. }) if given == param
      ),
      "{query}"
    );
  }
  let Ok(Route::Project(_, ProjectPage::Usage(today))) =
    Route::parse(&format!("/project/{PROJECT}/usage?period=today"))
  else {
    panic!("today's usage is a route");
  };
  assert_eq!(
    today.group,
    GroupBy::Hour,
    "today reads by hour, as pressing Today shows it"
  );
  let everywhere = Route::Usage(Box::new(UsageQuery {
    filter: orchester_core::usage::UsageFilter {
      projects: vec![project()],
      ..Default::default()
    },
    ..usage_query()
  }));
  assert_eq!(
    Route::parse(&everywhere.path()),
    Ok(everywhere.clone()),
    "every project's usage"
  );
  assert!(everywhere.path().starts_with(&format!(
    "/usage?from=2026-10-01&to=2026-10-15&group=agent&project={PROJECT}"
  )));
  assert_eq!(everywhere.nav_screen(), Screen::AllProjects);
  assert!(
    matches!(
      Route::parse(&format!("/project/{PROJECT}/usage?project={PROJECT}")),
      Err(RouteError::InvalidParam { param, .. }) if param == "project"
    ),
    "a project's page is the project's"
  );
  assert_eq!(
    Route::parse(&format!("/project/{PROJECT}/usage?tab=table")),
    Err(RouteError::UnexpectedParam {
      param: "tab".to_owned()
    })
  );
}
