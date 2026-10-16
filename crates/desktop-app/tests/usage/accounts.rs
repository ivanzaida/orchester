//! The page by connected account (`Usage · many accounts` `u2cb1M`) on the `usage` seed's five accounts: the
//! legend's series in order with Other accounts and No account last with its ⓘ, the table taking the whole
//! row with more than three series, and the Account filter grouped by CLI with *No account*.

use orchester_core::{
  account::AccountCli,
  time::Timestamp,
  usage::{DayRange, Period, local_date},
};
use orchester_workspace::usage::UsageQuery;

use crate::harness::Seeded;

#[test]
fn usage_accounts_legend_goes_by_account_with_other_and_no_account_last() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  let legend = |page: &mut Seeded, series: usize| page.text(&format!("usage-tokens-legend-{series}"));
  assert!(legend(&mut page, 0).starts_with("Codex · Client"), "the first account");
  assert!(legend(&mut page, 1).starts_with("Codex · Default"));
  assert!(legend(&mut page, 2).starts_with("Claude · Work"));
  assert!(
    legend(&mut page, 3).starts_with("Other accounts · 2"),
    "Personal and Claude's Default: {}",
    legend(&mut page, 3)
  );
  assert!(
    legend(&mut page, 4).starts_with("No account"),
    "the reviewer and unrecorded runs"
  );
  assert!(page.shows("usage-no-account-info"), "No account's ⓘ");
}

#[test]
fn usage_accounts_table_takes_the_row_with_more_than_three_series() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-tokens-view-table");
  let header = page.text("usage-tokens-table-header");
  for heading in ["CODEX", "CLIENT", "WORK", "OTHER", "2 ACCOUNTS", "NO ACCOUNT"] {
    assert!(header.contains(heading), "{heading} in {header}");
  }
  assert!(
    page.shows("usage-limits-row"),
    "Plan limits goes under the table (FdC7Y)"
  );
}

#[test]
fn usage_accounts_filter_groups_by_cli_and_chooses_no_account() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-filter-account");
  assert!(page.shows("usage-filter-account-menu-group-codex"));
  assert!(page.shows("usage-filter-account-menu-group-claude_code"));
  assert_eq!(page.text("usage-filter-account-option-0-label"), "Client");
  assert_eq!(page.text("usage-filter-account-option-5-label"), "No account");
  page.click("usage-filter-account-option-5");
  assert!(page.query().filter.no_account, "No account chosen");
  page.until("the chip", |page| page.shows("usage-filter-account-chip-0"));
  assert_eq!(page.text("usage-filter-account-label"), "Account · 1 of 6");
  assert_eq!(page.text("usage-filter-account-chip-0"), "Account No account");
  assert_eq!(
    page.text("usage-limits-none-title"),
    "No plan limits from the chosen accounts",
    "plan limits follow the Account filter; reports it leaves out still came"
  );
}

/// Three series: Codex's Default, Claude Code's Default (Other's one account) and No account.
fn three_series() -> UsageQuery {
  let mut query = UsageQuery::default();
  query.filter.accounts = vec![
    AccountCli::Codex.default_account(),
    AccountCli::ClaudeCode.default_account(),
  ];
  query.filter.no_account = true;
  query
}

#[test]
fn usage_accounts_narrow_table_shows_every_heading_and_cell_whole() {
  let mut page = Seeded::usage("usage", three_series());
  page.click("usage-tokens-view-table");
  assert!(!page.shows("usage-limits-row"), "three series stay beside Plan limits");
  let width = |page: &mut Seeded, cell: usize| page.bounds(&format!("usage-tokens-table-row-0-cell-{cell}")).2;
  for (cell, expected) in [(1, 84.0), (2, 84.0), (3, 84.0), (4, 84.0), (5, 36.0)] {
    assert!(
      (width(&mut page, cell) - expected).abs() < 0.5,
      "column {cell} is {expected} (d3lc3)"
    );
  }
  let header = page.text("usage-tokens-table-header");
  assert!(header.contains("NO ACCOUNT"), "{header}");
  assert_headings_fit(&mut page, "usage-tokens-table", 6);
  let mut row = 0;
  while page.shows(&format!("usage-tokens-table-row-{row}")) {
    for cell in 0..6 {
      assert_whole(&mut page, &format!("usage-tokens-table-row-{row}-cell-{cell}"));
    }
    row += 1;
  }
  assert!(row >= 7, "a week's rows");
  for cell in 0..6 {
    assert_whole(&mut page, &format!("usage-tokens-table-total-cell-{cell}"));
  }
  let footnote = page.text("usage-tokens-footnote");
  assert!(
    footnote.starts_with("Exact counts as the CLIs reported them. Today counts up to "),
    "{footnote}"
  );
}

#[test]
fn usage_accounts_readings_from_yesterday_keep_their_window_whole() {
  let yesterday = local_date(Timestamp::now()).pred_opt().expect("a day before today");
  let query = UsageQuery {
    period: Period::Custom(DayRange::new(yesterday, yesterday).expect("a day")),
    ..UsageQuery::default()
  };
  let mut page = Seeded::usage("usage", query);
  page.click("usage-limits-view-table");
  assert!(
    page.text("usage-limits-table-row-0-cell-0").starts_with("yesterday "),
    "a reading from yesterday"
  );
  let width = |page: &mut Seeded, cell: usize| page.bounds(&format!("usage-limits-table-row-0-cell-{cell}")).2;
  assert!(
    (width(&mut page, 0) - 96.0).abs() < 0.5,
    "REPORTED fits yesterday 19:17"
  );
  assert!(
    (width(&mut page, 1) - 140.0).abs() < 0.5,
    "WINDOW keeps the design's 140 for Codex · Default · weekly"
  );
  assert_readings_whole(&mut page);
}

#[test]
fn usage_accounts_resets_on_a_two_digit_day_stay_whole() {
  // Oct 8, 2026 at noon UTC: Codex's weekly window resets on Oct 12 and Claude's on Oct 10.
  let now = Timestamp::from_unix_millis(1_791_460_800_000);
  let today = local_date(now);
  let first = today - chrono::Days::new(6);
  let query = UsageQuery {
    period: Period::Custom(DayRange::new(first, today).expect("a week")),
    ..UsageQuery::default()
  };
  let mut page = Seeded::usage_at("usage", now, query);
  page.click("usage-limits-view-table");
  let width = page.bounds("usage-limits-table-row-0-cell-3").2;
  assert!((width - 84.0).abs() < 0.5, "RESETS is 84: {width}");
  let mut resets = Vec::new();
  let mut row = 0;
  while page.shows(&format!("usage-limits-table-row-{row}")) {
    resets.push(page.text(&format!("usage-limits-table-row-{row}-cell-3")));
    row += 1;
  }
  assert!(
    resets
      .iter()
      .any(|reset| reset.starts_with("Oct 1") && reset.contains(", ")),
    "a reset on a two-digit day: {resets:?}"
  );
  assert_readings_whole(&mut page);
}

/// Every heading of `table` lies inside its cell, so nothing cuts it, and its cell ends where the column's
/// cells do.
fn assert_headings_fit(page: &mut Seeded, table: &str, columns: usize) {
  for index in 0..columns {
    let heading = format!("{table}-heading-{index}");
    let cell = page.bounds(&heading);
    let words = page.bounds(&format!("{heading}-words"));
    assert!(
      words.0 >= cell.0 - 0.5 && words.0 + words.2 <= cell.0 + cell.2 + 0.5,
      "{heading} ({}) {words:?} runs out of its cell {cell:?}",
      page.text(&heading)
    );
    let data = page.bounds(&format!("{table}-row-0-cell-{index}"));
    assert!(
      (cell.0 + cell.2 - data.0 - data.2).abs() < 0.5,
      "{heading} ends where its column does: {cell:?} over {data:?}"
    );
  }
}

/// Every reading's cells whole, and the headings over them.
fn assert_readings_whole(page: &mut Seeded) {
  assert_headings_fit(page, "usage-limits-table", 5);
  let mut row = 0;
  while page.shows(&format!("usage-limits-table-row-{row}")) {
    for cell in 0..4 {
      assert_whole(page, &format!("usage-limits-table-row-{row}-cell-{cell}"));
    }
    row += 1;
  }
  assert!(row > 0, "readings");
}

/// The text in `cell` is whole: one cut with an ellipsis is as wide as its cell.
fn assert_whole(page: &mut Seeded, cell: &str) {
  let text = page.bounds(&format!("{cell}-text")).2;
  let room = page.bounds(cell).2;
  assert!(
    text < room - 0.01,
    "{cell} ({}) is cut: {text} of {room}",
    page.text(cell)
  );
}
