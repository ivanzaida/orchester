//! What the Usage page's controls do (`Usage · interactions` `p732q`): a filter's menu chooses and a chip lets
//! go, a *Where tokens go* row filters by its profile, the period and *Group by* change the query, *Custom…*
//! takes typed days and refuses a reversed range, and the warning threshold is saved for every project. Each
//! change is the route's query, in the tab's current entry.

use orchester_core::usage::{GroupBy, Period};
use orchester_workspace::usage::UsageQuery;

use crate::harness::Seeded;

#[test]
fn usage_filter_menus_choose_and_chips_let_go() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-filter-cli");
  assert!(page.shows("usage-filter-cli-option-0") && page.shows("usage-filter-cli-option-1"));
  // A press outside an open menu closes it; the next one opens another.
  page.click("usage-filter-cli");
  page.click("usage-filter-model");
  assert!(page.shows("usage-filter-model-menu-search"), "Model's menu searches");
  let model = page.text("usage-filter-model-option-0-label");
  page.click("usage-filter-model-option-0");
  assert_eq!(page.query().filter.models, [model.as_str().into()]);
  page.until("the chip", |page| page.shows("usage-filter-model-chip-0"));
  assert_eq!(
    page.text("usage-filter-model-label"),
    "Model · 1 of 3",
    "the agents' two and the reviewer's"
  );
  // A choice keeps the menu open for the next; a press outside closes it first.
  page.click("usage-filter-model");
  page.click("usage-filter-model-chip-0-remove");
  assert!(page.query().filter.is_empty());
  page.click("usage-filter-tag");
  assert!(
    page.text("usage-filter-tag-menu").contains("runs"),
    "tags with their runs"
  );
}

#[test]
fn usage_breakdown_rows_filter_by_their_profile() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  let name = page.text("usage-breakdown-profiles-row-0-name");
  let profile = name.split(" · ").next().expect("a profile").to_owned();
  page.click("usage-breakdown-profiles-row-0");
  assert_eq!(page.query().filter.profiles, [profile.as_str().into()]);
  page.until("the chip", |page| page.shows("usage-filter-profile-chip-0"));
  assert_eq!(page.text("usage-filter-profile-chip-0-value"), profile);
}

#[test]
fn usage_period_and_grouping_change_the_query() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-period-month");
  assert_eq!(page.query().period, Period::Month);
  page.click("usage-period-today");
  assert_eq!(
    (page.query().period, page.query().group),
    (Period::Today, GroupBy::Hour)
  );
  page.click("usage-period-week");
  page.click("usage-group-by");
  page.click("usage-group-day");
  assert_eq!(page.query().group, GroupBy::Day);
}

#[test]
fn usage_custom_range_refuses_a_reversed_range_and_shows_a_typed_one() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-period-custom");
  page.until("the range", |page| page.shows("usage-custom"));
  page.window.set("usage-custom-from", "2026-09-30");
  page.window.set("usage-custom-to", "2026-09-01");
  page.until("the error", |page| page.shows("usage-custom-error"));
  assert_eq!(page.text("usage-custom-error"), "The range ends before it starts.");
  page.window.set("usage-custom-from", "2026-09-01");
  page.window.set("usage-custom-to", "2026-09-30");
  page.until("the button", |page| {
    page.text("usage-custom-show") == "Show Sep 1 – Sep 30"
  });
  page.click("usage-custom-show");
  assert_eq!(
    orchester_workspace::links::with_usage_query(String::new(), &page.query()),
    "?from=2026-09-01&to=2026-09-30",
    "the range, by day as before"
  );
}

#[test]
fn usage_warning_threshold_is_saved_for_every_project() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-limits-menu");
  page.until("the threshold", |page| page.shows("usage-threshold"));
  page.window.set("usage-threshold", "0");
  page.until("the refusal", |page| page.shows("usage-threshold-error"));
  page.window.set("usage-threshold", "70");
  page.until("the save", |page| page.shows("usage-threshold-saved"));
  let workspace = page.window.launch.workspace().expect("the session started");
  assert_eq!(workspace.usage_warning_threshold().expect("read").percent(), 70);
}
