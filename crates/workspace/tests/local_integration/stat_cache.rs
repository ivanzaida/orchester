//! An edit that keeps a file's size and modification time is hidden from Git's stat cache. Only the hashed check
//! before an operation finds it; the refusal then makes Git show it, so the person sees it and can review again.

use std::{
  fs,
  time::{Duration, SystemTime},
};

use orchester_core::integration::local::{LocalError, LocalPath};
use orchester_exec::local_git::LocalGitError;
use orchester_workspace::tasks::local_integration::{IntegrationError, read};

use super::{
  privacy::Vault,
  support::{Fixture, git},
};

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
fn local_stat_hidden_edit_is_refused_then_shown_and_a_refresh_recovers() {
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
    "Git's stat cache hides the edit"
  );
  let tree = session.selected_commit_tree(std::slice::from_ref(&note)).expect("tree");
  let refused = session.commit(std::slice::from_ref(&note), &tree, "selected commit");
  assert!(
    matches!(
      refused,
      Err(IntegrationError::Git(LocalGitError::State(LocalError::Stale)))
    ),
    "{refused:?}"
  );
  assert!(
    git(root, &["status", "--porcelain"]).contains("M local.txt"),
    "the refusal makes Git show the edit"
  );
  let view = read(root, fixture.run, None, &Vault(None)).expect("review read");
  assert!(
    view.checkout.files.iter().any(|file| file.path.as_str() == "local.txt"),
    "the page lists it"
  );
  session.refresh().expect("review again");
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
