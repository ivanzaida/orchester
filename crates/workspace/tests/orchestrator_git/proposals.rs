//! What can lose work is only proposed — also with the person's switch off — and changes nothing until it is applied;
//! applying it does exactly what the card showed, and is refused once the repository moved since.

use orchester_core::orchestrator::git::{GitOperation, GitProposal};
use serde_json::json;

use super::scratch::{Scratch, code, refused};

#[test]
fn orchestrator_git_reset_is_proposed_with_what_it_loses_and_applied_as_shown() {
  let scratch = Scratch::new();
  scratch.commit("later.txt", "later\n", "Later work");
  let later = scratch.git(&["rev-parse", "--short", "HEAD"]);
  scratch.write("README.md", "uncommitted\n");
  let before = scratch.head("HEAD");

  let change = scratch.proposed(&json!({ "operation": { "kind": "reset", "to": "HEAD~1", "mode": "hard" } }));
  assert!(matches!(
    change.operation,
    GitOperation::Propose(GitProposal::Reset { .. })
  ));
  assert_eq!(change.loses, [format!("{later} Later work"), "M\tREADME.md".to_owned()]);
  assert!(
    change.commands[0].starts_with("git reset --hard "),
    "{:?}",
    change.commands
  );
  assert_eq!(scratch.head("HEAD"), before, "proposing changes nothing");
  assert_eq!(scratch.read("README.md"), "uncommitted\n");

  let told = scratch.apply(&change).expect("the person applied it");
  assert_eq!(told["done"], true);
  assert_eq!(scratch.head("HEAD"), scratch.head("origin/main"));
  assert_eq!(scratch.read("README.md"), "Kontur\n");
}

#[test]
fn orchestrator_git_proposal_is_refused_once_the_repository_moved() {
  let scratch = Scratch::new();
  scratch.git(&["switch", "-c", "feature"]);
  scratch.commit("feature.txt", "feature\n", "Feature");
  scratch.git(&["switch", "main"]);
  let change = scratch.proposed(&json!({ "operation": { "kind": "delete_branch", "branch": "feature" } }));
  assert_eq!(change.loses.len(), 1, "its unmerged commit: {:?}", change.loses);

  scratch.git(&["switch", "feature"]);
  scratch.commit("feature.txt", "more\n", "More feature");
  scratch.git(&["switch", "main"]);
  let error = scratch.apply(&change).expect_err("it moved");
  assert_eq!(code(&error), "changed_since_proposed");
  assert!(
    !scratch.git(&["branch", "--list", "feature"]).is_empty(),
    "the branch stays"
  );

  let again = scratch.proposed(&json!({ "operation": { "kind": "delete_branch", "branch": "feature" } }));
  scratch.apply(&again).expect("applied as shown now");
  assert!(scratch.git(&["branch", "--list", "feature"]).is_empty());
}

#[test]
fn orchestrator_git_force_push_leases_what_the_card_read() {
  let scratch = Scratch::new();
  scratch.commit("pushed.txt", "pushed\n", "Pushed");
  scratch.git(&["push", "origin", "main"]);
  let pushed = scratch.git(&["rev-parse", "--short", "HEAD"]);
  scratch.git(&["reset", "--hard", "HEAD~1"]);
  scratch.commit("instead.txt", "instead\n", "Instead");

  let change = scratch.proposed(&json!({ "operation": { "kind": "force_push" } }));
  assert_eq!(
    change.loses,
    [format!("{pushed} Pushed")],
    "the remote's commit it pushes over"
  );
  assert!(
    change.commands[0].starts_with("git push --force-with-lease=refs/heads/main:"),
    "{:?}",
    change.commands
  );
  scratch.apply(&change).expect("the person applied it");
  assert_eq!(
    scratch.git_in(&scratch.origin(), &["rev-parse", "main"]),
    scratch.head("HEAD")
  );
}

#[test]
fn orchestrator_git_clean_discard_and_abort_are_proposals() {
  let scratch = Scratch::new();
  scratch.write("scratch.log", "noise\n");
  let clean = scratch.proposed(&json!({ "operation": { "kind": "clean" } }));
  assert_eq!(clean.loses, ["scratch.log"]);
  assert!(scratch.root().join("scratch.log").exists(), "nothing deleted yet");
  scratch.apply(&clean).expect("applied");
  assert!(!scratch.root().join("scratch.log").exists());

  scratch.write("README.md", "edited\n");
  let discard = scratch.proposed(&json!({ "operation": { "kind": "discard_changes", "paths": ["README.md"] } }));
  assert_eq!(discard.loses, ["M\tREADME.md"]);
  scratch.apply(&discard).expect("applied");
  assert_eq!(scratch.read("README.md"), "Kontur\n");

  let nothing = scratch.call("git_propose", &json!({ "operation": { "kind": "abort_merge" } }), true);
  assert_eq!(refused(nothing), "nothing_in_progress");
  let nothing = scratch.call("git_propose", &json!({ "operation": { "kind": "clean" } }), true);
  assert_eq!(refused(nothing), "nothing_to_clean");
}

#[test]
fn orchestrator_git_proposal_is_refused_after_an_edit_of_what_it_discards() {
  let scratch = Scratch::new();
  scratch.write("README.md", "first edit\n");
  let change = scratch.proposed(&json!({ "operation": { "kind": "reset", "to": "HEAD", "mode": "hard" } }));
  assert_eq!(change.loses, ["M\tREADME.md"]);
  // The same file, the same list of what it loses, but other words than the card was read with.
  scratch.write("README.md", "second edit, worth keeping\n");
  let error = scratch.apply(&change).expect_err("the work changed");
  assert_eq!(code(&error), "changed_since_proposed");
  assert_eq!(scratch.read("README.md"), "second edit, worth keeping\n");

  // So does a switch of branch at the same commit.
  let change = scratch.proposed(&json!({ "operation": { "kind": "reset", "to": "HEAD", "mode": "hard" } }));
  scratch.git(&["switch", "-c", "elsewhere"]);
  assert_eq!(
    code(&scratch.apply(&change).expect_err("HEAD moved")),
    "changed_since_proposed"
  );
}

#[test]
fn orchestrator_git_remote_tag_deletion_leases_the_tag_the_card_read() {
  let scratch = Scratch::new();
  scratch.git(&["tag", "-a", "v1", "-m", "First"]);
  scratch.git(&["push", "origin", "v1"]);
  let change = scratch.proposed(&json!({ "operation": { "kind": "delete_tag", "tag": "v1", "remote": "origin" } }));
  assert!(
    change.commands[0].starts_with("git push --force-with-lease=refs/tags/v1:"),
    "{:?}",
    change.commands
  );
  scratch.apply(&change).expect("the person applied it");
  assert!(
    scratch.git_in(&scratch.origin(), &["tag"]).is_empty(),
    "the remote's tag is gone"
  );

  // The remote's tag moved since the card was read: the lease refuses, and the remote keeps it.
  scratch.git(&["push", "origin", "v1"]);
  let change = scratch.proposed(&json!({ "operation": { "kind": "delete_tag", "tag": "v1", "remote": "origin" } }));
  let other = scratch.dir.path().join("other");
  let origin = scratch.origin().to_string_lossy().into_owned();
  scratch.git_in(scratch.dir.path(), &["clone", &origin, "other"]);
  scratch.git_in(&other, &["tag", "-f", "-a", "v1", "-m", "Moved"]);
  scratch.git_in(&other, &["push", "--force", "origin", "v1"]);
  let moved = scratch.git_in(&scratch.origin(), &["rev-parse", "refs/tags/v1"]);
  let error = scratch.apply(&change).expect_err("the lease is stale");
  assert_eq!(code(&error), "git_failed");
  assert_eq!(scratch.git_in(&scratch.origin(), &["rev-parse", "refs/tags/v1"]), moved);

  let unknown = scratch.call(
    "git_propose",
    &json!({ "operation": { "kind": "delete_tag", "tag": "nope", "remote": "origin" } }),
    false,
  );
  assert_eq!(refused(unknown), "git_failed");
}

#[test]
fn orchestrator_git_switch_discarding_lists_untracked_files_in_the_way() {
  let scratch = Scratch::new();
  scratch.git(&["switch", "-c", "feature"]);
  scratch.commit("notes.txt", "the branch's notes\n", "Notes");
  scratch.git(&["switch", "main"]);
  scratch.write("notes.txt", "the person's own notes\n");
  scratch.write("README.md", "edited\n");
  let change = scratch.proposed(&json!({ "operation": { "kind": "switch_discarding", "branch": "feature" } }));
  assert_eq!(change.loses, ["M\tREADME.md", "untracked\tnotes.txt"]);
}

#[test]
fn orchestrator_git_clean_never_touches_orchester_data() {
  let scratch = Scratch::new();
  scratch.write("build/out.o", "object\n");
  scratch.write(".gitignore", "build/\n");
  scratch.git(&["add", ".gitignore"]);
  scratch.git(&["commit", "-m", "Ignore build"]);
  let change = scratch.proposed(&json!({ "operation": { "kind": "clean", "directories": true, "ignored": true } }));
  assert_eq!(change.loses, ["build/"], "never .orchester/");
  scratch.apply(&change).expect("applied");
  assert!(!scratch.root().join("build").exists());
  assert!(scratch.root().join(".orchester").is_dir(), "Orchester's data stays");
  orchester_store::ProjectDb::open(&scratch.root()).expect("and still opens");
}
