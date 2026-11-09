//! The Tests card on a task's Summary (`07 · Summary · Tests` `qN2FK`) on the QA seed's `summary-tests` scenario:
//! what attempt 2's Test step kept — its layers with their counts and times, the failed tests with file:line, their
//! message and links, the flaky tests marked, and the files grouped — in place of that step's report among the
//! run's output; a press on a link opens the stored file (recorded, not opened: the test asks for it), and a run
//! without a Test step has no card.

use std::path::Path;

use crate::page::Page;

const TITLE: &str = "Invite people to a team server";

#[test]
fn summary_tests_card_shows_what_the_test_step_kept() {
  let mut page = Page::seeded("summary-tests");
  page.window.tree.resize(1440, 2400);
  page.open_titled(TITLE);
  assert_eq!(page.text("task-tests-title"), "Tests");
  let meta = page.text("task-tests-meta");
  assert!(meta.starts_with("04 Test · attempt 2 · "), "{meta}");
  assert_eq!(page.text("task-tests-failed-badge"), "2 failed");
  assert_eq!(page.text("task-tests-flaky-badge"), "2 flaky");
  for (layer, suites, counts, time) in [
    ("lint", "rust-lint, web-lint", "no problems", "38 s"),
    ("unit", "rust-unit, web-unit", "412 passed 1 flaky", "2m 05s"),
    ("e2e", "web-e2e, team-e2e", "46 passed 2 failed 1 flaky", "3m 57s"),
  ] {
    let id = format!("task-tests-layer-{layer}");
    assert_eq!(page.text(&format!("{id}-name")), layer);
    assert_eq!(page.text(&format!("{id}-suites")), suites, "{layer}");
    assert_eq!(page.text(&format!("{id}-counts")), counts, "{layer}");
    assert_eq!(page.text(&format!("{id}-time")), time, "{layer}");
  }
  assert_eq!(
    page.text("task-tests-failed-detail"),
    "2 · re-run once, failed again · returned to 03 Implement"
  );
  assert_eq!(
    page.text("task-tests-failed-0-name"),
    "invites › accepts an invite twice"
  );
  assert_eq!(
    page.text("task-tests-failed-0-location"),
    "web/team/e2e/invites.spec.ts:48"
  );
  assert_eq!(
    page.text("task-tests-failed-0-message"),
    "Error: expect(received).toBe(expected) Expected: 409   Received: 500"
  );
  assert_eq!(page.text("task-tests-failed-1-name"), "teams › renames a team");
  assert_eq!(page.text("task-tests-failed-0-artifact-screenshot"), "Screenshot");
  assert_eq!(page.text("task-tests-failed-0-artifact-trace"), "Trace");
  // The flaky tests, the quarantined one marked so (`NmMGj`, `n5K1Ej`).
  assert_eq!(
    page.text("task-tests-flaky-0"),
    "store::tests::concurrent_writes crates/cloud/src/store/tests.rs:212 passed on re-run"
  );
  assert_eq!(
    page.text("task-tests-flaky-1"),
    "billing › shows the invoice list web/cloud/e2e/billing.spec.ts:31 passed on re-run quarantined"
  );
  assert_eq!(page.text("task-tests-artifacts-screenshot-name"), "Screenshots");
  assert_eq!(
    page.text("task-tests-artifacts-screenshot-count"),
    "2 · at the failures"
  );
  assert_eq!(page.text("task-tests-artifacts-trace-name"), "Traces");
  // The card says what the Test step found: its report has no row of its own among the output.
  assert_eq!(page.text("task-output-head"), "Output 0");
}

#[test]
fn summary_tests_card_links_open_the_stored_files() {
  let mut page = Page::seeded("summary-tests");
  page.window.tree.resize(1440, 2400);
  let services = page.window.launch.services().expect("the session started").clone();
  services.links.record_instead();
  page.open_titled(TITLE);
  page.click("task-tests-failed-0-artifact-screenshot");
  page.click("task-tests-failed-1-artifact-trace");
  page.click("task-tests-artifacts-screenshot");
  let recorded = services.links.recorded();
  let [screenshot, trace, folder] = recorded.as_slice() else {
    panic!("three files opened: {recorded:?}");
  };
  for (opened, ends) in [
    (screenshot, "invites-accepts-an-invite-twice/test-failed-1.png"),
    (trace, "teams-renames-a-team/trace.zip"),
  ] {
    let path = Path::new(opened);
    assert!(path.is_file(), "{opened} is the stored file");
    assert!(opened.replace('\\', "/").ends_with(ends), "{opened}");
    assert!(opened.contains(".orchester"), "{opened} is in the run's storage");
  }
  // The screenshots of two tests lie in two folders: the group opens the folder that holds them both.
  assert!(Path::new(folder).is_dir(), "{folder}");
  assert!(folder.replace('\\', "/").ends_with("/tests"), "{folder}");
}

#[test]
fn summary_a_run_without_a_test_step_has_no_tests_card() {
  let mut page = Page::seeded("task-states");
  for state in ["failed", "ready for review"] {
    page.open(state);
    assert!(page.has("task-run-head"), "{state}: the run shows");
    assert!(!page.has("task-tests"), "{state}: no Test step, no card");
  }
}
