//! The page by connected account (`Usage · many accounts` `u2cb1M`) on the `usage` seed's five accounts: the
//! legend's series in order with Other accounts and No account last with its ⓘ, the table taking the whole
//! row with more than three series, and the Account filter grouped by CLI with *No account*.

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
