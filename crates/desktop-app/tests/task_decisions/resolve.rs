//! Resolving a merge's conflicts on the *Not merged into main* card: it offers *Local integration*, *Resolve in
//! editor* and *Ask an agent to resolve*; *Resolve in editor* merges main into the run's worktree with git's markers
//! and starts the editor (a stand-in that writes down what it was given); *Continue* refuses while markers remain and
//! commits the merge once they are gone, after which *Merge into main* merges; *Abort* puts the worktree back; a
//! worktree with uncommitted work is not merged into. No runner is connected here: asking an agent waits for one.

use std::{
  fs,
  path::{Path, PathBuf},
  process,
};

use orchester_core::run::Run;
use orchester_workspace::tasks::resolve::Editor;

use super::{OFFLINE, Page, checkout_page, merge::latest_run};

const CONFLICTING: &str = "ready for review, conflicts with main";
const FILES: [&str; 2] = ["src/webhooks/sender.rs", "src/webhooks/settings.rs"];

/// A stand-in editor in `folder` that writes the arguments it was started with to `editor-args.txt` there.
fn stand_in(folder: &Path) -> (Editor, PathBuf) {
  let record = folder.join("editor-args.txt");
  write_stand_in(folder, &record);
  (stand_in_program(folder), record)
}

/// The stand-in [`stand_in`] writes in `folder`, as the editor *Resolve in editor* starts.
fn stand_in_program(folder: &Path) -> Editor {
  Editor::Named(folder.join(STAND_IN))
}

/// The stand-in's file name: a script the platform runs.
#[cfg(windows)]
const STAND_IN: &str = "stand-in-editor.cmd";
#[cfg(not(windows))]
const STAND_IN: &str = "stand-in-editor";

#[cfg(windows)]
fn write_stand_in(folder: &Path, record: &Path) {
  fs::write(
    folder.join(STAND_IN),
    format!("@echo %* > \"{}\"\r\n", record.display()),
  )
  .expect("the stand-in");
}

#[cfg(not(windows))]
fn write_stand_in(folder: &Path, record: &Path) {
  use std::os::unix::fs::PermissionsExt;
  let program = folder.join(STAND_IN);
  fs::write(&program, format!("#!/bin/sh\necho \"$@\" > '{}'\n", record.display())).expect("the stand-in");
  fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("executable");
}

fn worktree(page: &Page, run: &Run) -> PathBuf {
  page.repo.join(".orchester/worktrees").join(run.id().to_string())
}

/// `git <args>` in `dir`; what it printed.
fn git_in(dir: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The Summary of the conflicting task after *Merge into main* stopped at its conflicts.
fn conflicted(editor: Option<(Editor, PathBuf)>) -> (Page, Run, Option<PathBuf>) {
  let (mut page, record) = match editor {
    Some((editor, record)) => (Page::seeded_with_editor("decisions", editor), Some(record)),
    None => (Page::seeded("decisions"), None),
  };
  let run = latest_run(&page, CONFLICTING);
  page.open(CONFLICTING, "task-merge");
  page.merge_until(("task-merge", "task-notice-title"), "Not merged into main");
  page.until_says("task-merge-conflicts", FILES[0]);
  (page, run, record)
}

/// Waits until the merge waits in the worktree for the person.
fn resolving(page: &mut Page) {
  page
    .window
    .until_within(super::MERGE_WAIT, "the merge to wait in the worktree", |window| {
      window.texts("task-resolving-title").is_some()
    });
}

/// The person resolves the conflicts in `worktree` one file after the other, as their editor would save them: Continue
/// waits while a marker is left and says where, and Merge into main waits for the merge in the worktree. The editor
/// tells Orchester nothing: the page reads the worktree again on a timer.
fn resolve_by_hand(page: &mut Page, worktree: &Path) {
  assert!(page.locked("task-resolve-continue"));
  assert_eq!(
    page.text("task-resolve-continue-reason"),
    format!(
      "Conflict markers are still in {}, {}. Resolve them, save the files, then continue.",
      FILES[0], FILES[1]
    )
  );
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge waits while Orchester merges into the run's worktree for you to resolve its conflicts."
  );
  git_in(worktree, &["checkout", "--ours", "--", FILES[0]]);
  page.window.until("one file left with markers", |window| {
    window.tree.tick_timers();
    window.texts("task-resolve-continue-reason").is_some_and(|texts| {
      texts
        .join(" ")
        .starts_with("Conflict markers are still in src/webhooks/settings.rs.")
    })
  });
  git_in(worktree, &["checkout", "--ours", "--", FILES[1]]);
  page.window.until("Continue once no marker is left", |window| {
    window.tree.tick_timers();
    window.texts("task-resolve-continue-reason").is_none()
  });
}

#[test]
fn decision_resolve_a_conflict_offers_three_ways_and_the_editor_merges_into_the_worktree() {
  let _checkout = checkout_page();
  let folder = tempfile::tempdir().expect("a folder for the stand-in");
  let (mut page, run, record) = conflicted(Some(stand_in(folder.path())));
  let record = record.expect("the stand-in's record");
  let checkout = git_in(&page.repo, &["rev-parse", "HEAD"]);
  assert!(page.has("task-open-local-integration"));
  assert!(page.has("task-resolve-in-editor"));
  assert!(page.has("task-resolve-ask-agent"));
  let program = match &stand_in_program(folder.path()) {
    Editor::Named(program) => program.file_name().expect("a file").to_string_lossy().into_owned(),
    other => panic!("a named stand-in, not {other:?}"),
  };
  assert_eq!(
    page.text("task-resolve-editor"),
    format!("Merges main into the run's worktree and opens it with its conflicting files in {program}.")
  );
  // Asking an agent goes to the runner, which is not connected here.
  assert!(page.locked("task-resolve-ask-agent"));
  assert_eq!(page.text("task-resolve-agent-reason"), OFFLINE);

  page.click("task-resolve-in-editor");
  resolving(&mut page);
  let worktree = worktree(&page, &run);
  for file in FILES {
    let text = fs::read_to_string(worktree.join(file)).expect("the conflicted file");
    assert!(text.contains("<<<<<<<"), "{file} holds git's markers");
  }
  assert!(page.text("task-resolving-files").contains(FILES[1]));
  page
    .window
    .until("the stand-in editor to write what it got", |_| record.is_file());
  let given = fs::read_to_string(&record).expect("the stand-in's record");
  assert!(given.contains(&worktree.display().to_string()), "{given}");
  assert!(given.contains("sender.rs"), "{given}");
  assert_eq!(
    git_in(&page.repo, &["rev-parse", "HEAD"]),
    checkout,
    "the checkout is untouched"
  );

  resolve_by_hand(&mut page, &worktree);
  assert!(page.text("task-resolving-files").contains("Resolved"));
  page.click("task-resolve-continue");
  page
    .window
    .until_within(super::MERGE_WAIT, "the Ready card again", |window| {
      window.texts("task-resolving-title").is_none() && window.texts("task-merge").is_some()
    });
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
  git_in(&worktree, &["merge-base", "--is-ancestor", &checkout, "HEAD"]);
  page.click("task-merge");
  page
    .window
    .until_within(super::MERGE_WAIT, "the merge to end", |window| {
      window
        .texts("task-notice-title")
        .is_some_and(|texts| texts.join(" ").contains("erged into"))
    });
  let said = page.text("task-notice");
  assert!(said.starts_with("Merged into main"), "{said}");
}

#[test]
fn decision_resolve_abort_puts_the_worktree_back() {
  let _checkout = checkout_page();
  let folder = tempfile::tempdir().expect("a folder for the stand-in");
  let (mut page, run, _) = conflicted(Some(stand_in(folder.path())));
  let worktree = worktree(&page, &run);
  let before = git_in(&worktree, &["rev-parse", "HEAD"]);
  page.click("task-resolve-in-editor");
  resolving(&mut page);
  page.click("task-resolve-abort");
  page
    .window
    .until_within(super::MERGE_WAIT, "the conflict card again", |window| {
      window.texts("task-resolving-title").is_none() && window.texts("task-resolve-in-editor").is_some()
    });
  assert_eq!(git_in(&worktree, &["rev-parse", "HEAD"]), before);
  assert_eq!(git_in(&worktree, &["status", "--porcelain"]), "");
}

#[test]
fn decision_resolve_refuses_a_worktree_with_uncommitted_work() {
  let _checkout = checkout_page();
  let folder = tempfile::tempdir().expect("a folder for the stand-in");
  let (mut page, run, record) = conflicted(Some(stand_in(folder.path())));
  let worktree = worktree(&page, &run);
  fs::write(worktree.join("agent-notes.md"), "Not committed.\n").expect("uncommitted work");
  page.click("task-resolve-in-editor");
  page.until_says("task-resolve-failed", "agent-notes.md");
  assert!(!page.has("task-resolving-title"));
  assert!(!record.expect("the stand-in's record").exists(), "no editor started");
}

#[test]
fn decision_resolve_without_an_editor_says_so() {
  let _checkout = checkout_page();
  let (mut page, ..) = conflicted(None);
  assert!(page.locked("task-resolve-in-editor"));
  assert_eq!(
    page.text("task-resolve-editor"),
    "No editor was found on this computer: install VS Code's code command, or name your editor in ORCHESTER_EDITOR."
  );
}
