//! The Changes tab's states: no checkpoint, too large, worktree removed, a line shortened, a file in both
//! groups counted once, a worktree read again, and changes that could not be read until Try again.

use std::fs;

use lurq::node::color::Color;
use orchester_app::routes::ChangeSource;

use super::Page;

#[test]
fn changes_states_no_checkpoint_too_large_removed_and_line_shortened() {
  let mut page = Page::open();
  page.changes("KON-16", "changes-no-checkpoint");
  assert_eq!(page.tab_label(), "Changes", "no count before the first checkpoint");
  assert!(page.texts("changes-toolbar").contains(&"no checkpoint yet".to_owned()));

  page.changes("KON-25", "changes-too-large");
  let too_large = page.texts("changes-too-large").concat();
  assert!(too_large.contains("Too large to show here"), "{too_large}");
  assert!(too_large.contains("20,001"), "measured, not shown: {too_large}");
  assert!(!page.shows("changes-files"));
  assert_eq!(page.tab_label(), "Changes 1", "the files measured");

  page.changes("KON-26", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("too large to show"), "{notice}");
  assert!(
    page.shows("changes-group-checkpointed"),
    "the checkpointed files still show"
  );
  assert_eq!(
    page.tab_label(),
    "Changes 1+",
    "the checkpointed files, marked as not all"
  );

  page.changes("KON-1", "changes-notice");
  assert!(page.texts("changes-notice").concat().contains("worktree was removed"));
  page.wait_for("changes-group-checkpointed");
  assert!(!page.shows("changes-group-worktree"));

  page.changes("KON-28", "changes-group-worktree");
  assert_eq!(page.texts("changes-group-checkpointed")[1], "2");
  assert_eq!(page.texts("changes-group-worktree")[1], "3");
  assert_eq!(page.tab_label(), "Changes 3", "a file in both groups counts once");
  // A file with removed lines only has its removed count alone, in `danger`.
  let retries = page.texts("changes-worktree-file:src/webhooks/retries.rs");
  assert_eq!(retries[2..], ["−1"], "{retries:?}");
  let removed = page.painted_all("−1");
  assert!(!removed.is_empty());
  assert!(
    removed.iter().all(|color| *color == Color::from_hex("#F0605A")),
    "{removed:?}"
  );

  page.open_file("KON-27", "src/webhooks/format.rs", ChangeSource::Worktree);
  page.wait_for("changes-line-cut");
  assert_eq!(page.texts("changes-line-cut"), ["Line shortened"]);
  assert!(
    !page.shows("changes-group-checkpointed"),
    "no checkpoint: only the worktree's group"
  );
}

#[test]
fn changes_reads_the_worktree_again_and_recovers_with_try_again() {
  let mut page = Page::open();
  page.changes("KON-12", "changes-group-worktree");
  page.wait_for("changes-worktree-file:tests/webhooks/jitter.rs");
  let worktree = page.worktree("KON-12");
  fs::write(worktree.join("notes.md"), "draft\n").expect("write into the worktree");
  // Leaving the tab and coming back reads the worktree again.
  page.go("KON-12/summary");
  page.wait_for("task-summary");
  page.changes("KON-12", "changes-worktree-file:notes.md");
  assert_eq!(page.texts("changes-group-worktree")[1], "3");
  page.window.until("the count of 7", |window| {
    window
      .texts("task-tab-changes-label")
      .is_some_and(|texts| texts.concat() == "Changes 7")
  });

  // A worktree git cannot read: a notice with Try again over the checkpointed files.
  let link = fs::read_to_string(worktree.join(".git")).expect("the worktree's link");
  fs::write(worktree.join(".git"), "gitdir: missing\n").expect("break the worktree's link");
  page.go("KON-12/summary");
  page.wait_for("task-summary");
  page.changes("KON-12", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("Couldn't read what the worktree holds"), "{notice}");
  assert!(notice.contains("git could not read the run's worktree"), "{notice}");
  assert!(page.shows("changes-group-checkpointed"));
  assert_eq!(page.tab_label(), "Changes 4+");
  fs::write(worktree.join(".git"), link).expect("mend the worktree's link");
  page.click("changes-worktree-try-again");
  page.wait_for("changes-worktree-file:notes.md");
  assert!(!page.shows("changes-notice"));
  assert_eq!(page.tab_label(), "Changes 7");
}

#[test]
fn changes_that_cannot_be_read_say_why_until_try_again() {
  let mut page = Page::open();
  let objects = page.repo.join(".git").join("objects");
  let moved = page.repo.join(".git").join("objects-away");
  fs::rename(&objects, &moved).expect("hide the repository's objects");
  page.changes("KON-12", "changes-failed");
  assert!(
    page
      .texts("changes-failed")
      .concat()
      .contains("Couldn't read the changes")
  );
  assert_eq!(page.tab_label(), "Changes", "no count while the changes cannot be read");
  fs::rename(&moved, &objects).expect("bring the objects back");
  page.click("changes-try-again");
  page.wait_for("changes-group-checkpointed");
  page.wait_for("changes-group-worktree");
  assert_eq!(page.tab_label(), "Changes 6");
}

#[test]
fn changes_of_an_unreadable_run_a_missing_worktree_and_a_run_without_checkpoint() {
  let mut page = Page::open();
  page.changes("KON-24", "changes-unreadable");
  let card = page.texts("changes-unreadable").concat();
  assert!(card.contains("Summary lists the task’s runs"), "{card}");
  assert!(!page.shows("changes-no-run"));

  // A run that has not ended without its worktree: an error, not a clean worktree.
  let worktree = page.worktree("KON-16");
  let away = worktree.with_extension("away");
  fs::rename(&worktree, &away).expect("move the worktree away");
  page.changes("KON-16", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("worktree is missing"), "{notice}");
  assert!(!page.shows("changes-no-checkpoint"));
  fs::rename(&away, &worktree).expect("bring the worktree back");

  // Without a checkpoint, a worktree that cannot be read does not speak of checkpointed files.
  let worktree = page.worktree("KON-27");
  fs::write(worktree.join(".git"), "gitdir: missing\n").expect("break the worktree's link");
  page.changes("KON-27", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("has no checkpoint yet"), "{notice}");
  assert!(
    notice.starts_with("Couldn't read what the run's worktree holds"),
    "{notice}"
  );
  assert!(!notice.contains("beyond the checkpoint"), "{notice}");
  assert!(!notice.contains("checkpointed files still show"), "{notice}");
}
