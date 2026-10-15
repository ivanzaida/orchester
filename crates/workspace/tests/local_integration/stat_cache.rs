//! An edit that keeps a file's size and modification time is hidden from Git's stat cache where the repository
//! does not trust change times. The hashed check before an operation refuses it without writing the index, so the
//! person's own staging is never overwritten, and the next commit candidate reviews the checkout as it is.

use std::{
  fs,
  time::{Duration, SystemTime},
};

use orchester_core::integration::local::{LocalError, LocalPath};
use orchester_exec::local_git::LocalGitError;
use orchester_workspace::tasks::local_integration::IntegrationError;

use super::support::{Fixture, git};

/// Rewrites `local.txt` with as many bytes as before and gives it back its modification time. The entry Git
/// recorded is a minute older than its index, so Git trusts it without reading the file again.
fn hidden_edit(fixture: &Fixture) {
  let path = fixture.root().join("local.txt");
  let set = |modified| {
    fs::File::options()
      .write(true)
      .open(&path)
      .expect("file")
      .set_modified(modified)
      .expect("mtime");
  };
  let old = SystemTime::now() - Duration::from_secs(60);
  set(old);
  git(fixture.root(), &["update-index", "--refresh"]);
  fs::write(&path, "BASE\n").expect("same-size edit");
  set(old);
}

#[test]
fn local_stat_hidden_edit_is_refused_without_writing_the_index_and_a_new_review_recovers() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  // A write changes a file's ctime, which Git compares unless told not to, as many repositories on shared or
  // synced folders are.
  git(root, &["config", "core.trustctime", "false"]);
  fs::write(root.join("note.txt"), "selected\n").expect("untracked");
  let note = LocalPath::new("note.txt").expect("path");
  let mut session = fixture.open();
  hidden_edit(&fixture);
  assert!(
    !git(root, &["status", "--porcelain"]).contains("local.txt"),
    "Git's own stat cache hides the edit"
  );
  // The person stages other content for it meanwhile; nothing of Orchester's writes an earlier entry back over it.
  fs::write(root.join("staged.txt"), "STAGED\n").expect("content to stage");
  let blob = git(root, &["hash-object", "-w", "staged.txt"]);
  fs::remove_file(root.join("staged.txt")).expect("only its object");
  git(
    root,
    &["update-index", "--cacheinfo", &format!("100644,{blob},local.txt")],
  );
  let index = fs::read(root.join(".git/index")).expect("index");
  let refused = session.selected_commit_tree(std::slice::from_ref(&note));
  assert!(
    matches!(
      refused,
      Err(IntegrationError::State(LocalError::Stale) | IntegrationError::Git(LocalGitError::State(LocalError::Stale)))
    ),
    "{refused:?}"
  );
  assert_eq!(
    fs::read(root.join(".git/index")).expect("index"),
    index,
    "the refusal writes nothing to the index"
  );
  assert_eq!(
    git(root, &["show", ":local.txt"]),
    "STAGED",
    "the person's staging stays"
  );
  // A new commit candidate reviews the checkout as it is, hashed.
  session.review_current().expect("review again");
  let tree = session.selected_commit_tree(std::slice::from_ref(&note)).expect("tree");
  session
    .commit(std::slice::from_ref(&note), &tree, "selected commit")
    .expect("the commit after the new review");
  assert_eq!(
    git(root, &["show", "HEAD:local.txt"]),
    "base",
    "the edit stays the person's"
  );
  assert_eq!(git(root, &["show", ":local.txt"]), "STAGED");
}

#[test]
fn local_stat_hidden_edit_alone_is_refused_and_reviewed_by_the_next_candidate() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  git(root, &["config", "core.trustctime", "false"]);
  fs::write(root.join("note.txt"), "selected\n").expect("untracked");
  let note = LocalPath::new("note.txt").expect("path");
  let mut session = fixture.open();
  hidden_edit(&fixture);
  let index = fs::read(root.join(".git/index")).expect("index");
  let refused = session.selected_commit_tree(std::slice::from_ref(&note));
  assert!(refused.is_err(), "the hashed check refuses the unreviewed edit");
  assert_eq!(
    fs::read(root.join(".git/index")).expect("index"),
    index,
    "and writes nothing to the index"
  );
  session.review_current().expect("the next candidate reviews again");
  let tree = session.selected_commit_tree(std::slice::from_ref(&note)).expect("tree");
  session
    .commit(std::slice::from_ref(&note), &tree, "selected commit")
    .expect("the commit after the new review");
  assert_eq!(
    git(root, &["show", "HEAD:local.txt"]),
    "base",
    "the edit stays the person's"
  );
}
