//! The Usage page's states (`Usage · project · dark` `CFRdj`, `Usage · table view` `d3lc3`, `Usage · states`
//! `cK32U`): the charts and tables of the seeded week, every chart as a table, reading, no runs today,
//! filters that match nothing, and usage that cannot be read.

use orchester_core::usage::Period;
use orchester_workspace::usage::UsageQuery;

use crate::harness::Seeded;

const DASH: &str = "—";

#[test]
fn usage_page_charts_the_seeded_week() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  assert_eq!(page.text("usage-summary-runs-value"), "10");
  assert!(page.text("usage-summary-runs-change").contains("more than"));
  assert!(page.text("usage-readback").contains("compared with"));
  let legend = page.text("usage-tokens-legend");
  assert!(legend.contains("Codex") && legend.contains("Claude"), "{legend}");
  assert!(page.shows("usage-limits-chart"));
  assert!(page.text("usage-limits-legend").contains("Claude · weekly"));
  let footnote = page.text("usage-limits-footnote");
  assert!(
    footnote.contains("Hatched: ") && footnote.contains("no Claude data before"),
    "{footnote}"
  );
  assert!(page.text("usage-breakdown-profiles").contains("Builder · Implement"));
  assert!(page.text("usage-breakdown-models").contains("claude-sonnet-5-5"));
  assert!(page.text("usage-breakdown-pipelines").contains("Feature delivery"));
  for row in 0..5 {
    assert!(page.shows(&format!("usage-expensive-tasks-row-{row}")), "row {row}");
  }
  assert!(!page.shows("usage-expensive-tasks-row-5"), "five rows, then Show all");
  assert_eq!(
    page.text("usage-expensive-shown"),
    "5 of 10 tasks with runs in the period"
  );
  assert!(page.shows("usage-cache-chart"));
  assert!(
    page.text("usage-edits-with-median") != DASH,
    "the seed's indexed runs edited"
  );
  assert!(page.text("usage-failed-runs").starts_with("2 of 10"));
}

#[test]
fn usage_page_reads_every_chart_as_a_table() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-tokens-view-table");
  let total = page.text("usage-tokens-table-total");
  assert!(total.starts_with("Total") && total.ends_with(" 10"), "{total}");
  assert!(page.text("usage-tokens-table-header").contains("CODEX"));
  page.click("usage-tokens-view-chart");
  assert!(page.shows("usage-tokens-chart"));
  // Each table back to its chart, so the cards under it stay in the window; Plan limits' readings last.
  for (view, table) in [
    ("usage-breakdown-view", "usage-breakdown-profiles-table"),
    ("usage-cache-view", "usage-cache-table"),
    ("usage-edits-view", "usage-edits-table"),
    ("usage-failed-view", "usage-failed-table"),
    ("usage-limits-view", "usage-limits-table"),
  ] {
    page.click(&format!("{view}-table"));
    assert!(page.shows(table), "{view} shows {table}");
    if view != "usage-limits-view" {
      page.click(&format!("{view}-chart"));
      assert!(!page.shows(table), "{view} returns to its chart");
    }
  }
}

#[test]
fn usage_page_shows_dashes_and_what_it_reads_while_it_reads() {
  let mut page = Seeded::prepared("usage", true, |_| {});
  page.go(UsageQuery::default());
  page.until("the page", |page| page.shows("usage-tokens-reading"));
  assert_eq!(page.text("usage-summary-spent-value"), DASH);
  assert!(!page.shows("usage-summary-spent-change"), "no change while reading");
  assert_eq!(
    page.text("usage-tokens-reading"),
    "Reading token usage from Kontur’s runs…"
  );
  assert_eq!(page.text("usage-limits-reading"), "Reading plan limit snapshots…");
}

#[test]
fn usage_page_says_when_nothing_ran_today() {
  let today = UsageQuery {
    period: Period::Today,
    ..UsageQuery::default()
  };
  let mut page = Seeded::usage("usage-empty", today);
  assert_eq!(page.text("usage-no-runs-title"), "No runs in Kontur today");
  assert!(
    page
      .text("usage-no-runs-detail")
      .contains("Kontur’s last run reported usage")
  );
  assert_eq!(page.text("usage-summary-spent-value"), "0");
  assert_eq!(page.text("usage-summary-cache-value"), DASH);
  assert!(page.text("usage-summary-runs-change").starts_with("yesterday"));
  assert_eq!(page.text("usage-summary-time-detail"), "no agent worked today");
  assert!(page.text("usage-limits-caption").starts_with("last reported"));
  page.click("usage-show-week");
  assert_eq!(page.query().period, Period::Week);
}

#[test]
fn usage_page_names_filters_that_match_nothing() {
  let mut query = UsageQuery::default();
  query.filter.models.push("claude-opus-5-5".into());
  query.filter.pipelines.push("Quick fix".into());
  let mut page = Seeded::usage("usage", query);
  assert_eq!(page.text("usage-unmatched-title"), "No usage matches these filters");
  assert!(
    page
      .text("usage-unmatched-detail")
      .contains("matches Model: claude-opus-5-5, Pipeline: Quick fix. Kontur has 10 runs in this period."),
    "{}",
    page.text("usage-unmatched-detail")
  );
  assert_eq!(page.text("usage-filter-model-chip-0"), "Model claude-opus-5-5");
  assert_eq!(page.text("usage-summary-runs-detail"), "no matching runs");
  assert!(
    page
      .text("usage-limits-caption")
      .starts_with("follow the CLI filter only")
  );
  page.click("usage-clear-filters-empty");
  assert!(page.query().filter.is_empty());
}

#[test]
fn usage_page_says_why_it_could_not_read_the_usage() {
  let mut page = Seeded::prepared("usage", false, |repo| {
    let db = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("the project database");
    db.execute("UPDATE invocation_edit_progress SET body = 'not json'", [])
      .expect("damage the turns");
  });
  page.go(UsageQuery::default());
  page.until("the failure", |page| page.shows("usage-load-failed"));
  assert_eq!(page.text("usage-load-failed-title"), "Usage could not be read");
  assert!(page.shows("usage-retry"));
}

#[test]
fn usage_page_names_the_years_of_a_range_across_new_year() {
  let range = orchester_core::usage::DayRange::new(
    chrono::NaiveDate::from_ymd_opt(2025, 12, 28).expect("a day"),
    chrono::NaiveDate::from_ymd_opt(2026, 1, 3).expect("a day"),
  )
  .expect("a range");
  let query = UsageQuery {
    period: Period::Custom(range),
    ..UsageQuery::default()
  };
  let mut page = Seeded::usage("usage", query);
  let readback = page.text("usage-readback");
  assert!(readback.starts_with("Dec 28, 2025 – Jan 3, 2026"), "{readback}");
}
