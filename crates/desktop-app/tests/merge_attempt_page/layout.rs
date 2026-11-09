//! The attempt page's layout in the Review v2 design (ORC-915, 13a to 13h): a long title wraps beside *Copy details*,
//! the bar says what each state means and offers its one action, and the conflicts' bar holds the editor, the agent
//! field and the primary action.

use super::Page;

/// A title long enough to wrap (`LONG_TITLE` of the seed's `merge-attempts`).
const LONG_TITLE: &str = "Add retry with exponential backoff and full jitter to the webhook sender, and keep every attempt visible in the endpoint's delivery log";

#[test]
fn merge_attempt_page_a_long_title_wraps_beside_copy_details_and_never_runs_under_it() {
  let mut page = Page::new();
  page.open("Add retry to webhook sender");
  let bounds = |page: &mut Page, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let short = bounds(&mut page, "merge-attempt-title");
  page.open(LONG_TITLE);
  let (long, copy) = (
    bounds(&mut page, "merge-attempt-title"),
    bounds(&mut page, "merge-attempt-copy-details"),
  );
  assert!(
    long.height > short.height * 1.5,
    "it wraps: {} against {}",
    long.height,
    short.height
  );
  assert!(
    long.x + long.width <= copy.x,
    "it ends before Copy details begins: {} and {}",
    long.x + long.width,
    copy.x
  );
  assert!(
    (long.y - copy.y).abs() < copy.height,
    "Copy details stays at the top of the header: {} and {}",
    long.y,
    copy.y
  );
}

#[test]
fn merge_attempt_page_the_bar_says_what_the_state_means_and_offers_its_one_action() {
  let mut page = Page::new();
  for (title, strip, bar, body, action) in [
    (
      "Add retry to webhook sender",
      "What stopped it",
      "Nothing was merged",
      "once the other operation ends",
      "merge-attempt-try-again",
    ),
    (
      "Per-endpoint rate limits",
      "What stopped it",
      "The merge queue stopped this run",
      "puts it back in the merge queue",
      "merge-attempt-try-again",
    ),
    (
      "Delivery dashboard",
      "What stopped it",
      "Your code is in main, Orchester has not recorded it",
      "the run stays Ready to merge",
      "merge-attempt-record-merge",
    ),
    (
      "Rate limit per endpoint",
      "What stopped it",
      "Nothing was merged",
      "once the disk answers",
      "merge-attempt-try-again",
    ),
    (
      "Payload size guard",
      "What stopped it",
      "1 file still holds the merge's version",
      "Put files back restores them",
      "merge-attempt-put-back",
    ),
    (
      "Per-endpoint secrets",
      "What stopped it",
      "2 files conflict with main",
      "then it's back for your review",
      "merge-attempt-ask-agent",
    ),
  ] {
    page.open(title);
    assert_eq!(page.window.text("merge-attempt-what-title"), strip, "{title}");
    assert_eq!(page.window.text("merge-attempt-bar-title"), bar, "{title}");
    assert!(
      page.window.text("merge-attempt-bar-body").contains(body)
        || page.window.text("merge-attempt-bar-reason").contains(body)
        || page.shows("merge-attempt-bar-reason"),
      "{title}: the bar says what it means"
    );
    assert!(page.shows(action), "{title}: {action}");
  }
  page.open("Delivery log per endpoint");
  assert_eq!(page.window.text("merge-attempt-what-title"), "What happened");
  assert!(!page.shows("merge-attempt-bar"), "a merged run has no bar");
}

#[test]
fn merge_attempt_page_the_bar_of_conflicts_holds_the_editor_and_the_agent_beside_the_primary_action() {
  let mut page = Page::new();
  page.open("Per-endpoint secrets");
  let left = |page: &mut Page, id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .x
  };
  let (editor, ask) = (
    left(&mut page, "merge-attempt-resolve-in-editor"),
    left(&mut page, "merge-attempt-ask-agent"),
  );
  assert!(editor < ask, "Resolve in editor comes before Ask the agent to resolve");
  assert_eq!(page.window.text("merge-attempt-open-in-review"), "Open in Review");
  assert!(
    !page.shows("merge-attempt-resolve-step"),
    "the agent field replaces the step select"
  );
}
