//! A task's memory (`Task · Memory · dark` `a0PLO`, the view its Summary's properties link to) on KON-12 of the
//! `memory` scenario: the records grouped with the proposals first, the latest handoff flagged and the earlier ones
//! folded, *Next agent receives*, and every change made in place — approve, review with edits, reject, edit, pin,
//! delete, a new record — with the search and filters.

use orchester_core::memory::{MemoryScope, MemoryStatus};

use super::harness::Page;

fn kon_12() -> Page {
  let mut page = Page::seeded("memory");
  page.task_memory("KON-12");
  page
}

#[test]
fn memory_task_summary_links_to_the_task_s_memory_with_its_active_count() {
  let mut page = Page::seeded("memory");
  page.task_summary("KON-12");
  page.until("the count of its notes", |window| {
    window.texts("task-memory-link-label") == Some(vec!["10 memory notes".to_owned()])
  });
  page.window.press("task-memory-link");
  page.until("the task's memory", |window| window.texts("memory-list").is_some());
  assert_eq!(page.text("memory-count"), "10 active · 2 waiting");
  assert_eq!(
    page.text("task-tab-summary"),
    "Summary",
    "the memory is a view of the Summary's tab"
  );
}

#[test]
fn memory_task_tab_groups_the_records_with_the_waiting_ones_first() {
  let mut page = kon_12();
  let groups = page.text("memory-list");
  let order: Vec<usize> = [
    "WAITING FOR YOUR APPROVAL",
    "HANDOFFS",
    "DECISIONS",
    "WARNINGS",
    "FACTS",
  ]
  .iter()
  .map(|heading| groups.find(heading).unwrap_or_else(|| panic!("{heading} is shown")))
  .collect();
  assert!(
    order.windows(2).all(|pair| pair[0] < pair[1]),
    "in the design's order: {groups}"
  );
  assert_eq!(page.text("memory-count"), "10 active · 2 waiting");
  let latest = page.record("Returned the work to Implement");
  assert!(page.has(&format!("memory-record-{}-latest", latest.id())));
  assert_eq!(
    page.text(&format!("memory-record-{}-lists", latest.id())),
    "Completed Checked the 5 criteria against a91f3e0; ran cargo test -p webhooks. Open questions Should 503 honor \
     Retry-After as well as 429? Next agent should know 429 responses ignore Retry-After, and no test pins the \
     jitter bound."
  );
  let earlier = page.text("memory-earlier-handoffs");
  assert!(earlier.starts_with("Show 3 earlier handoffs"), "{earlier}");
  assert!(earlier.contains("01 Research"), "{earlier}");
  let research = page.record("The sender retries in a fixed loop");
  assert!(!page.has(&format!("memory-record-{}", research.id())), "folded");
  page.window.press("memory-earlier-handoffs");
  assert!(page.has(&format!("memory-record-{}", research.id())), "unfolded");
  let warning = page.record("Don't retry 4xx");
  assert!(page.has(&format!("memory-record-{}-pinned", warning.id())));
  assert!(
    page
      .text(&format!("memory-record-{}-meta", warning.id()))
      .starts_with("You · ")
  );
}

#[test]
fn memory_task_tab_says_what_the_next_agent_receives() {
  let mut page = kon_12();
  assert_eq!(page.text("memory-next-step"), "03 Implement · Builder");
  let given = page.text("memory-next-given");
  assert!(
    given.starts_with("LATEST HANDOFF 1 05 Review · Returned the work to Implement"),
    "{given}"
  );
  // The task's pinned warning and the project's pinned db.engine.
  assert!(given.contains("PINNED 2"), "{given}");
  assert!(given.contains("db.engine Project"), "{given}");
  assert!(given.contains("DECISIONS AND WARNINGS 3"), "{given}");
  let records = page.text("memory-next-records-value");
  assert!(records.ends_with("of 8"), "{records}");
  assert!(page.text("memory-next-tokens-value").ends_with("of 3k"));
  assert_eq!(page.text("memory-next-task-saving"), "Task memory Save after approval");
  assert!(
    !page.text("memory-next").contains("per endpoint"),
    "a waiting proposal is never given"
  );
  assert_eq!(
    page.text("memory-next-settings"),
    "Change in Project settings › Context ↗"
  );
  let context = orchester_app::routes::Route::Project(
    page.project,
    orchester_app::routes::ProjectPage::Settings(orchester_app::routes::ProjectSettingsTab::Context),
  );
  page.press_until("memory-next-settings", "Project settings › Context", |window| {
    window.navigation().route_now() == context
  });
}

#[test]
fn memory_task_tab_approves_a_proposal_that_supersedes_a_decision() {
  let mut page = kon_12();
  let proposal = page.record("Retry budget is per endpoint");
  let old = page.record("Retry budget is a global constant");
  let earlier_attempt = page.record("Every attempt is recorded in webhook_attempts");
  let supersedes = page.text(&format!("memory-proposal-{}-supersedes", proposal.id()));
  assert_eq!(
    supersedes,
    "Supersedes 1 active record · webhooks.retry.budget by Planner"
  );
  let id = proposal.id();
  page.press_until(&format!("memory-proposal-{id}-approve"), "the approval", |window| {
    window.texts(&format!("memory-proposal-{id}")).is_none()
  });
  assert_eq!(page.stored(id).expect("kept").status(), MemoryStatus::Active);
  assert_eq!(page.stored(old.id()).expect("kept").status(), MemoryStatus::Superseded);
  // Implement's latest attempt now defines the step's decisions: its first attempt's is deprecated (core's rule).
  assert_eq!(
    page.stored(earlier_attempt.id()).expect("kept").status(),
    MemoryStatus::Deprecated
  );
  page.until("the count after the approval", |window| {
    window.text("memory-count") == "9 active · 1 waiting"
  });
}

#[test]
fn memory_task_tab_reviews_a_proposal_and_saves_the_edits_with_the_approval() {
  let mut page = kon_12();
  let id = page.record("Retry budget is per endpoint").id();
  page.window.press(&format!("memory-proposal-{id}-edit"));
  let editor = format!("memory-editor-{id}");
  assert!(page.text(&editor).starts_with("Review proposed decision"));
  assert!(
    page
      .text(&format!("{editor}-replaces"))
      .starts_with("Approving replaces the active decision webhooks.retry.budget")
  );
  let facts = page.text(&format!("{editor}-facts"));
  assert!(facts.contains("Source 3 events in 03 Implement"), "{facts}");
  assert!(facts.contains("Agent's confidence High · set by Builder"), "{facts}");
  page
    .window
    .set(&format!("{editor}-text"), "Retry budget is per endpoint, at most 10.");
  page.press_until(&format!("{editor}-save"), "the approval", |window| {
    window.texts(&format!("memory-editor-{id}")).is_none()
  });
  let approved = page.stored(id).expect("kept");
  assert_eq!(approved.status(), MemoryStatus::Active);
  assert_eq!(approved.content().text(), "Retry budget is per endpoint, at most 10.");
  assert!(approved.edited_at().is_some());
}

#[test]
fn memory_task_tab_rejects_a_proposal() {
  let mut page = kon_12();
  let id = page.record("Retry tests use the fake clock").id();
  page.press_until(&format!("memory-proposal-{id}-reject"), "the rejection", |window| {
    window.texts(&format!("memory-proposal-{id}")).is_none()
  });
  assert_eq!(page.stored(id).expect("kept").status(), MemoryStatus::Rejected);
}

#[test]
fn memory_task_tab_edits_pins_and_deletes_a_record_in_place() {
  let mut page = kon_12();
  let id = page.record("Exponential backoff from 500 ms").id();
  let row = format!("memory-record-{id}");
  assert!(
    !page.has(&format!("{row}-actions")),
    "actions show under the pointer only"
  );
  page.hover(&row);
  page.window.press(&format!("{row}-edit"));
  let editor = format!("memory-editor-{id}");
  assert!(page.text(&editor).starts_with("Edit decision"));
  page.window.set(
    &format!("{editor}-text"),
    "Exponential backoff from 500 ms; a longer Retry-After wins.",
  );
  page.press_until(&format!("{editor}-save"), "the edit", |window| {
    window.texts(&format!("memory-editor-{id}")).is_none()
  });
  let edited = page.stored(id).expect("kept");
  assert_eq!(
    edited.content().text(),
    "Exponential backoff from 500 ms; a longer Retry-After wins."
  );
  page.until("the edited row", |window| {
    window
      .texts(&format!("memory-record-{id}-meta"))
      .is_some_and(|meta| meta.join(" ").contains("edited by you"))
  });
  page.hover(&row);
  page.press_until(&format!("{row}-pin"), "the pin", |window| {
    window.texts(&format!("memory-record-{id}-pinned")).is_some()
  });
  assert!(page.stored(id).expect("kept").is_pinned());
  page.hover(&row);
  page.window.press(&format!("{row}-delete"));
  let confirm = format!("memory-delete-{id}");
  page.until("the invocations it was given", |window| {
    window
      .texts(&format!("{confirm}-text"))
      .is_some_and(|text| text.join(" ").contains("no invocation was given it"))
  });
  assert!(page.text(&confirm).starts_with("Delete this record?"));
  page.press_until(&format!("{confirm}-confirm"), "the deletion", |window| {
    window.texts(&format!("memory-delete-{id}")).is_none()
  });
  assert!(page.stored(id).is_none());
}

#[test]
fn memory_task_tab_writes_a_new_record_to_the_task() {
  let mut page = kon_12();
  page.window.press("memory-new");
  assert!(page.text("memory-editor-new").starts_with("New record"));
  assert!(
    page
      .text("memory-editor-new-save-to-field")
      .contains("This task · KON-12")
  );
  page.window.set(
    "memory-editor-new-text",
    "Webhook endpoints are configured per project.",
  );
  page.press_until("memory-editor-new-save", "the new record", |window| {
    window.texts("memory-editor-new").is_none()
  });
  let created = page.record("Webhook endpoints are configured per project.");
  assert_eq!(created.status(), MemoryStatus::Active);
  assert_eq!(created.scope(), MemoryScope::Task(page.task("KON-12")));
}

#[test]
fn memory_task_tab_says_a_proposal_waits_at_its_top_right() {
  let mut page = kon_12();
  let id = page.record("Retry budget is per endpoint").id();
  let text = page.bounds(&format!("memory-proposal-{id}-text"));
  let waiting = page.bounds(&format!("memory-proposal-{id}-waiting"));
  assert!(
    waiting.x >= text.x + text.width,
    "the text stops short of the status: {text:?} {waiting:?}"
  );
  assert!(waiting.y <= text.y, "the status is on the row's first line");
}

#[test]
fn memory_task_tab_search_and_filters_say_what_matches() {
  let mut page = kon_12();
  page.window.set("memory-search", "oauth refresh");
  page.until("no match", |window| window.texts("memory-no-match").is_some());
  assert!(
    page
      .text("memory-no-match")
      .starts_with("No records match “oauth refresh”")
  );
  let (title, list) = (page.bounds("memory-no-match-title"), page.bounds("memory-list"));
  let middle = title.x + title.width / 2.0 - (list.x + list.width / 2.0);
  assert!(middle.abs() < 2.0, "centered across the list: {title:?} in {list:?}");
  assert!(
    title.y > list.y + list.height / 4.0,
    "in the middle of the list, not at its top: {title:?} in {list:?}"
  );
  page.window.set("memory-search", "");
  page.until("the list again", |window| window.texts("memory-no-match").is_none());
  page.window.press("memory-filter-status");
  page.window.press("memory-filter-status-menu-7");
  page.until("every status", |window| {
    window.text("memory-count").contains("not given")
  });
  assert_eq!(page.text("memory-filter-status"), "Status · All");
}

#[test]
fn memory_task_tab_keeps_a_long_record_in_place_under_the_pointer() {
  let mut page = kon_12();
  page.window.press("memory-new");
  let text = "## How each acceptance criterion is checked\n| Criterion | Check |\n|---|---|\n| Build on Windows; no \
              Bevy under `terrane_core` | `cargo build --workspace`; `cargo tree -p terrane_core` (also enforced in \
              check.ps1) |\n| Window opens with the Sculpt layout | `cargo test -p bevy_terrane_editor --test \
              shell_layout shell_regions_match_sculpt_layout`: lays out at 1920×1080 with `pass_headless` and asserts \
              region bounds (top bar 40, mode strip 64, tool panel 280, viewport 1276×1014, inspector 300, status \
              bar 26) |";
  page.window.set("memory-editor-new-text", text);
  page.press_until("memory-editor-new-save", "the new record", |window| {
    window.texts("memory-editor-new").is_none()
  });
  let row = format!("memory-record-{}", page.record("## How each acceptance criterion").id());
  page.until("the new record's row", |window| window.texts(&row).is_some());
  let before = page.bounds(&format!("{row}-text"));
  let line = page.bounds(&format!("{row}-meta")).height;
  assert!(before.height > 4.0 * line, "a long record wraps: {before:?}");
  assert!(!page.has(&format!("{row}-actions")), "no actions away from the pointer");
  page.hover(&row);
  assert!(
    page.has(&format!("{row}-actions")),
    "the actions show under the pointer"
  );
  let during = page.bounds(&format!("{row}-text"));
  assert_eq!(
    (before.x, before.y, before.width, before.height),
    (during.x, during.y, during.width, during.height),
    "the text keeps its width and its lines under the pointer"
  );
  let actions = page.bounds(&format!("{row}-actions"));
  assert!(
    actions.x >= during.x + during.width,
    "the actions stand beside the text, not over it: {during:?} {actions:?}"
  );
}
