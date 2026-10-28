//! The page by agent provider (decided 2026-10-28, in the look of `Usage · many accounts` `u2cb1M`) on the `usage`
//! seed's four connections: the legend's series in the Connections page's order with Other providers and *Earlier
//! runs* last with its ⓘ, never *No account*; the table taking the whole row with more than three series; the Provider
//! filter with *Earlier runs*; what *Plan limits*, which follow the CLI filter only, says; and the plan limit tables,
//! still by account.

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  id::{AgentAccountId, ConnectionId},
  time::Timestamp,
  usage::{DayRange, Period, local_date},
};
use orchester_workspace::usage::UsageQuery;

use crate::harness::Seeded;

#[test]
fn usage_providers_legend_goes_by_provider_with_other_and_earlier_runs_last() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  let legend = |page: &mut Seeded, series: usize| page.text(&format!("usage-tokens-legend-{series}"));
  assert!(
    legend(&mut page, 0).starts_with("Claude Code · Kontur"),
    "the Connections page's first: {}",
    legend(&mut page, 0)
  );
  assert!(
    legend(&mut page, 1).starts_with("Claude · DeepSeek"),
    "the API key's run is its provider's"
  );
  assert!(legend(&mut page, 2).starts_with("Codex · Client"));
  assert!(
    legend(&mut page, 3).starts_with("Codex · Kontur"),
    "Other's one provider reads as it: {}",
    legend(&mut page, 3)
  );
  assert!(
    legend(&mut page, 4).starts_with("Earlier runs"),
    "the reviews from before providers were recorded"
  );
  assert!(page.shows("usage-earlier-info"), "Earlier runs' ⓘ");
  assert!(page.ids("usage-no-account").is_empty(), "no No account anywhere");
}

#[test]
fn usage_providers_table_takes_the_row_with_more_than_three_series() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-tokens-view-table");
  let header = page.text("usage-tokens-table-header");
  assert!(!header.contains("NO ACCOUNT"), "{header}");
  // Each provider's heading on two lines, its CLI over the rest of its name (`CODEX` over `CLIENT` in FdC7Y).
  for (column, over, words) in [
    (1, Some("CLAUDE CODE"), "KONTUR"),
    (2, Some("CLAUDE"), "DEEPSEEK"),
    (3, Some("CODEX"), "CLIENT"),
    (4, Some("CODEX"), "KONTUR"),
    (5, None, "EARLIER RUNS"),
  ] {
    let heading = format!("usage-tokens-table-heading-{column}");
    assert_eq!(page.text(&format!("{heading}-text")), words, "{heading}");
    let shown = page.shows(&format!("{heading}-over"));
    assert_eq!(shown, over.is_some(), "{heading}'s line over it");
    if let Some(over) = over {
      assert_eq!(page.text(&format!("{heading}-over")), over, "{heading}");
    }
  }
  assert_tokens_headings_whole(&mut page, 5);
  assert!(
    page.shows("usage-limits-row"),
    "Plan limits goes under the table (FdC7Y)"
  );
}

/// The tokens table's series headings `1..=series` whole and apart: each line, at its column's right, leaves room
/// at its cell's left or runs past it (*EARLIER RUNS* in an 84 column), where a cut one fills it exactly, and ends
/// before the next heading starts.
fn assert_tokens_headings_whole(page: &mut Seeded, series: usize) {
  let table = "usage-tokens-table";
  for index in 1..=series {
    let heading = format!("{table}-heading-{index}");
    let cell = page.bounds(&heading);
    let next = page.bounds(&format!("{table}-heading-{}-words", index + 1));
    for line in ["words", "over"] {
      let id = format!("{heading}-{line}");
      if !page.shows(&id) {
        continue;
      }
      let shown = page.bounds(&id);
      assert!(
        shown.0 > cell.0 + 0.5 || shown.2 > cell.2 + 0.01,
        "{id} ({}) {shown:?} fills its cell {cell:?}, so it is cut",
        page.text(&id)
      );
      assert!(
        shown.0 + shown.2 <= next.0 + 0.5,
        "{id} ({}) {shown:?} runs into the next heading {next:?}",
        page.text(&id)
      );
    }
  }
}

#[test]
fn usage_providers_filter_lists_the_providers_then_earlier_runs() {
  let mut page = Seeded::usage("usage", UsageQuery::default());
  page.click("usage-filter-provider");
  assert!(page.shows("usage-filter-provider-menu-group"));
  assert_eq!(page.text("usage-filter-provider-option-1-label"), "Claude · DeepSeek");
  assert_eq!(page.text("usage-filter-provider-option-4-label"), "Earlier runs");
  page.click("usage-filter-provider-option-1");
  assert_eq!(page.query().filter.providers.len(), 1, "Claude · DeepSeek chosen");
  page.until("the chip", |page| page.shows("usage-filter-provider-chip-0"));
  assert_eq!(page.text("usage-filter-provider-label"), "Provider · 1 of 5");
  assert_eq!(page.text("usage-filter-provider-chip-0"), "Provider Claude · DeepSeek");
  assert!(
    page.shows("usage-limits-legend"),
    "a provider chooses no account's plan limits"
  );
}

/// The `usage` seed's connections by name.
fn connections(repo: &Path) -> Vec<(String, ConnectionId)> {
  let db = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("the project database");
  let mut statement = db.prepare("SELECT name, id FROM connections").expect("a query");
  statement
    .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
    .expect("the connections")
    .map(|row| {
      let (name, id) = row.expect("a connection");
      (name, id.parse().expect("an id"))
    })
    .collect()
}

#[test]
fn usage_providers_narrow_table_shows_every_heading_and_cell_whole() {
  let mut chosen = Vec::new();
  let mut page = Seeded::prepared("usage", false, |repo| {
    chosen = connections(repo)
      .into_iter()
      .filter(|(name, _)| name == "Claude · DeepSeek" || name == "Codex · Client")
      .map(|(_, id)| id)
      .collect();
  });
  let mut query = UsageQuery::default();
  query.filter.providers = chosen;
  query.filter.earlier = true;
  page.go(query);
  page.until("the usage", |page| page.shows("usage-tokens-chart"));
  page.click("usage-tokens-view-table");
  assert!(!page.shows("usage-limits-row"), "three series stay beside Plan limits");
  let width = |page: &mut Seeded, cell: usize| page.bounds(&format!("usage-tokens-table-row-0-cell-{cell}")).2;
  for (cell, expected) in [(1, 84.0), (2, 84.0), (3, 84.0), (4, 84.0), (5, 36.0)] {
    assert!(
      (width(&mut page, cell) - expected).abs() < 0.5,
      "column {cell} is {expected} (d3lc3)"
    );
  }
  assert_eq!(page.text("usage-tokens-table-heading-3-text"), "EARLIER RUNS");
  assert_tokens_headings_whole(&mut page, 3);
  let mut row = 0;
  while page.shows(&format!("usage-tokens-table-row-{row}")) {
    row += 1;
  }
  assert!(row >= 7, "a week's rows");
}

#[test]
fn usage_providers_readings_from_yesterday_keep_their_window_whole() {
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
fn usage_providers_resets_on_a_two_digit_day_stay_whole() {
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

#[test]
fn usage_providers_compact_plan_limits_name_the_cli_filter_that_hides_them() {
  let mut query = UsageQuery::default();
  query.filter.clis.push(AgentProvider::OpenAiApi);
  query.filter.models.push("claude-opus-5-5".into());
  let mut page = Seeded::usage("usage", query);
  assert!(page.shows("usage-unmatched"), "the filters match no run");
  assert_eq!(
    page.text("usage-limits-caption"),
    "no plan limits from the chosen CLIs",
    "an API reports no plan limits; the CLIs still reported theirs"
  );
}

#[test]
fn usage_providers_without_runs_plan_limits_follow_the_cli_filter() {
  let mut query = UsageQuery {
    period: Period::Today,
    ..UsageQuery::default()
  };
  query.filter.clis.push(AgentProvider::CodexCli);
  let mut page = Seeded::usage("usage-empty", query);
  assert!(page.shows("usage-no-runs"), "nothing ran today");
  let caption = page.text("usage-limits-caption");
  assert!(
    caption.starts_with("follow the CLI filter only"),
    "the windows shown are the chosen CLI's: {caption}"
  );
}

#[test]
fn usage_providers_plan_limit_windows_of_removed_accounts_keep_their_own_ids() {
  // Codex's reports and Claude's each on an account no longer connected: two removed accounts, each with a
  // 5-hour and a weekly window.
  let mut page = Seeded::prepared("usage", false, |repo| {
    let db = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("the project database");
    for (provider, account) in [
      ("codex", AgentAccountId::generate()),
      ("claude", AgentAccountId::generate()),
    ] {
      db.execute(
        "UPDATE plan_limit_snapshots SET account_id = ?1 WHERE provider LIKE ?2",
        [account.to_string(), format!("{provider}%")],
      )
      .expect("move the reports");
    }
  });
  page.go(UsageQuery::default());
  page.until("the plan limits", |page| page.shows("usage-limits-legend"));
  let windows: Vec<String> = page
    .ids("usage-limits-window-")
    .into_iter()
    .filter(|id| !id.ends_with("-value") && !id.ends_with("-resets"))
    .collect();
  assert_eq!(windows.len(), 4, "{windows:?}");
  let mut distinct = windows.clone();
  distinct.sort();
  distinct.dedup();
  assert_eq!(distinct.len(), windows.len(), "each window its own id: {windows:?}");
}
