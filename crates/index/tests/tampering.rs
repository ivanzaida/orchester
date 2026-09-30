//! An index an agent changed is not trusted: it is not built on and not handed out again, but built
//! anew; and an earlier index with files that have no outline is still built on.

mod support;

use std::fs;

use orchester_index::{BuildMode, Outcome};
use support::{Repo, files, read};

/// Appends `text` to `name` in the index `dir`, as an agent that clears the read-only flag could.
fn tamper(dir: &std::path::Path, name: &str, text: &str) {
  let file = dir.join(name);
  let mut permissions = fs::metadata(&file).expect("metadata").permissions();
  #[allow(
    clippy::permissions_set_readonly_false,
    reason = "the test plays an agent that makes the file writable"
  )]
  permissions.set_readonly(false);
  fs::set_permissions(&file, permissions).expect("make writable");
  let mut content = fs::read_to_string(&file).expect("read");
  content.push_str(text);
  fs::write(&file, content).expect("tamper");
}

#[test]
fn a_changed_index_is_neither_built_on_nor_handed_out_again() {
  let repo = Repo::new();
  let first = repo.head();
  let run = repo.run_with_worktree();
  let tainted = repo.indexes().ensure(&first, run).expect("index").path;
  tamper(&tainted, "outline/crates/app/src/lib.rs.md", "- L1-1 fn injected()\n");
  tamper(
    &tainted,
    "symbols.tsv",
    "injected\tcrates/app/src/lib.rs\t1\tfunction\t\n",
  );
  repo.write("README.md", "# Changed\n");
  repo.git(&["commit", "--quiet", "--all", "-m", "change"]);

  let second = repo.ensure(&repo.head());

  let Outcome::Built(report) = &second.outcome else {
    panic!("built");
  };
  assert_eq!(report.mode, BuildMode::Full, "the changed index is no base");
  assert!(!read(&second.path, "symbols.tsv").contains("injected"));
  assert!(!read(&second.path, "outline/crates/app/src/lib.rs.md").contains("injected"));

  let again = repo.indexes().ensure(&first, repo.run_with_worktree()).expect("index");
  assert!(
    matches!(again.outcome, Outcome::Built(_)),
    "the changed index is built again"
  );
  assert!(!read(&again.path, "symbols.tsv").contains("injected"));
  assert!(!read(&again.path, "outline/crates/app/src/lib.rs.md").contains("injected"));
}

#[test]
fn an_earlier_index_with_files_without_an_outline_is_still_built_on() {
  let repo = Repo::new();
  repo.write("web/min.js", &format!("{}\n", "function a(){return 1};".repeat(300)));
  repo.git(&["add", "web/min.js"]);
  repo.git(&["commit", "--quiet", "-m", "bundle"]);
  repo.ensure(&repo.head());
  repo.write("README.md", "# Changed\n");
  repo.git(&["commit", "--quiet", "--all", "-m", "change"]);
  let second = repo.head();

  let incremental = repo.ensure(&second);

  let Outcome::Built(report) = &incremental.outcome else {
    panic!("built");
  };
  assert!(
    matches!(report.mode, BuildMode::Incremental { .. }),
    "{:?}",
    report.mode
  );
  // README.md changed; the bundle has no outline, and its unchanged record is taken over.
  assert_eq!(report.read, 1);
  let incremental_files = files(&incremental.path);
  fs::remove_dir_all(repo.index_root()).expect("remove the indexes");
  assert_eq!(files(&repo.ensure(&second).path), incremental_files);
}
