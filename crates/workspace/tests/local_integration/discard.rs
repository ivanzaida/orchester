//! Discarding selected local changes of the main checkout: an untracked file goes to the trash, a changed or deleted
//! tracked file comes back from HEAD with the person's version in the trash first, a staged change leaves the index,
//! and nothing but the reviewed changes, inside the checkout, is touched. The trash is a folder of the test's own.

use std::{fs, path::PathBuf};

use orchester_core::integration::local::{Discard, LocalPath, LocalPhase};
use orchester_workspace::{
  accounts::trash::FolderTrash,
  tasks::local_integration::{DiscardFailure, LocalIntegration},
};

use super::support::{Fixture, git};

fn path(value: &str) -> LocalPath {
  LocalPath::new(value).expect("path")
}

/// A trash folder beside the fixture's repository, outside its checkout.
struct Bin(tempfile::TempDir);

impl Bin {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    Self(tempfile::Builder::new().prefix("trash-").tempdir_in(scratch).expect("trash"))
  }
  fn trash(&self) -> FolderTrash {
    FolderTrash::new(self.0.path())
  }
  fn holds(&self, name: &str) -> Option<Vec<u8>> {
    fs::read(self.0.path().join(name)).ok()
  }
  fn empty(&self) -> bool {
    fs::read_dir(self.0.path()).expect("trash folder").next().is_none()
  }
}

fn clean(fixture: &Fixture) -> bool {
  git(fixture.root(), &["status", "--porcelain"]).is_empty()
}

#[test]
fn local_integration_discard_moves_an_untracked_file_to_the_trash_and_prepares_delivery() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  fs::write(fixture.root().join("du.exe.stackdump"), b"crash dump").expect("junk");
  let mut session = fixture.open();
  let report = session
    .discard(&[path("du.exe.stackdump")], &bin.trash())
    .expect("discard");
  assert!(report.complete());
  assert_eq!(report.files()[0].discard, Discard::Trash);
  assert!(!fixture.root().join("du.exe.stackdump").exists());
  assert_eq!(bin.holds("du.exe.stackdump").as_deref(), Some(&b"crash dump"[..]));
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert!(session.operation().reviewed().clean());
  session.sync().expect("delivery continues: the fresh Check runs");
}

#[test]
fn local_integration_discard_restores_a_changed_file_and_keeps_the_persons_copy() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  fs::write(fixture.root().join("local.txt"), "my version\n").expect("edit");
  let mut session = fixture.open();
  let report = session.discard(&[path("local.txt")], &bin.trash()).expect("discard");
  assert!(report.complete());
  assert_eq!(report.files()[0].discard, Discard::RestoreKeepingCopy);
  assert_eq!(fs::read(fixture.root().join("local.txt")).expect("restored"), b"base\n");
  assert_eq!(bin.holds("local.txt").as_deref(), Some(&b"my version\n"[..]));
  assert!(clean(&fixture));
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
}

#[test]
fn local_integration_discard_restores_a_deleted_file_from_head() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  fs::remove_file(fixture.root().join("local.txt")).expect("delete");
  let mut session = fixture.open();
  let report = session.discard(&[path("local.txt")], &bin.trash()).expect("discard");
  assert!(report.complete());
  assert_eq!(report.files()[0].discard, Discard::Restore);
  assert_eq!(fs::read(fixture.root().join("local.txt")).expect("restored"), b"base\n");
  assert!(bin.empty(), "a deleted file has no version to keep");
  assert!(clean(&fixture));
}

#[test]
fn local_integration_discard_unstages_staged_changes_and_keeps_what_they_held() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  fs::write(fixture.root().join("local.txt"), "staged\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "staged then edited\n").expect("edit");
  fs::write(fixture.root().join("added.txt"), "new and staged\n").expect("new");
  git(fixture.root(), &["add", "added.txt"]);
  let mut session = fixture.open();
  let report = session
    .discard(&[path("local.txt"), path("added.txt")], &bin.trash())
    .expect("discard");
  assert!(report.complete());
  assert_eq!(report.files()[1].discard, Discard::Trash);
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "base");
  assert_eq!(fs::read(fixture.root().join("local.txt")).expect("restored"), b"base\n");
  assert!(!fixture.root().join("added.txt").exists());
  assert_eq!(bin.holds("local.txt").as_deref(), Some(&b"staged then edited\n"[..]));
  assert_eq!(bin.holds("added.txt").as_deref(), Some(&b"new and staged\n"[..]));
  assert!(clean(&fixture));
}

#[test]
fn local_integration_discard_keeps_the_rest_of_the_checkout() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  fs::write(fixture.root().join("junk.log"), "junk").expect("junk");
  fs::write(fixture.root().join("keep.txt"), "kept").expect("kept");
  let mut session = fixture.open();
  let report = session.discard(&[path("junk.log")], &bin.trash()).expect("discard");
  assert!(report.complete());
  assert_eq!(fs::read(fixture.root().join("keep.txt")).expect("kept"), b"kept");
  assert_eq!(session.operation().phase(), LocalPhase::Review);
  assert_eq!(session.operation().reviewed().files.len(), 1);
}

#[test]
fn local_integration_discard_refuses_paths_outside_the_reviewed_changes() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  for escape in ["../outside.txt", "a/../../outside.txt", ".git/config", ".orchester/orchester.db", "C:/outside.txt"] {
    assert!(LocalPath::new(escape).is_err(), "{escape} is no checkout path");
  }
  fs::write(fixture.root().join("junk.log"), "junk").expect("junk");
  fs::write(fixture.root().join("ignored.txt"), "ignored").expect("ignored");
  let mut session = fixture.open();
  // An ignored file and an unchanged tracked one are no reviewed change: nothing is discarded, not even the junk.
  for outside in ["ignored.txt", "task.txt"] {
    assert!(session.discard(&[path("junk.log"), path(outside)], &bin.trash()).is_err());
  }
  assert!(session.discard(&[path("junk.log"), path("junk.log")], &bin.trash()).is_err());
  assert!(session.discard(&[], &bin.trash()).is_err());
  for kept in ["junk.log", "ignored.txt", "task.txt"] {
    assert!(fixture.root().join(kept).exists(), "{kept} stays");
  }
  assert!(bin.empty());
}

#[test]
fn local_integration_discard_never_follows_a_link_out_of_the_checkout() {
  let (fixture, bin) = (Fixture::new(false), Bin::new());
  let outside = Bin::new();
  fs::write(outside.0.path().join("secret.txt"), "outside").expect("outside file");
  link(outside.0.path(), &fixture.root().join("linked"));
  let refused = LocalIntegration::open(fixture.root(), fixture.run).map_or(true, |mut session| {
    match session.discard(&[path("linked/secret.txt")], &bin.trash()) {
      Err(_) => true,
      Ok(report) => matches!(report.files()[0].failure, Some(DiscardFailure::Unsupported)),
    }
  });
  assert!(refused, "a path through a link is refused");
  assert_eq!(fs::read(outside.0.path().join("secret.txt")).expect("kept"), b"outside");
  assert!(bin.empty());
}

#[cfg(windows)]
fn link(target: &std::path::Path, at: &std::path::Path) {
  // mklink takes plain absolute paths, without `..` or the verbatim prefix.
  let plain = |path: &std::path::Path| {
    let full = path.canonicalize().expect("an existing folder");
    full.to_string_lossy().trim_start_matches(r"\\?\").to_owned()
  };
  let folder = plain(at.parent().expect("the checkout"));
  let name = at.file_name().expect("a name").to_string_lossy();
  let output = std::process::Command::new("cmd")
    .args(["/C", "mklink", "/J", &format!(r"{folder}\{name}"), &plain(target)])
    .output()
    .expect("mklink");
  assert!(output.status.success(), "a junction: {}", String::from_utf8_lossy(&output.stdout));
}

#[cfg(unix)]
fn link(target: &std::path::Path, at: &std::path::Path) {
  std::os::unix::fs::symlink(target, at).expect("a symlink");
}

/// Another program holding the file without letting it move: the system's reason is told, the file stays, and once
/// the program lets go, trying again discards it.
#[cfg(windows)]
#[test]
fn local_integration_discard_says_why_a_file_in_use_stayed_and_tries_again() {
  use std::os::windows::fs::OpenOptionsExt;

  let (fixture, bin) = (Fixture::new(false), Bin::new());
  let junk = fixture.root().join("in-use.log");
  fs::write(&junk, "held").expect("junk");
  fs::write(fixture.root().join("free.log"), "free").expect("junk");
  // Reading and writing are shared, moving is not.
  let held = fs::OpenOptions::new().read(true).share_mode(0x1 | 0x2).open(&junk).expect("held");
  let mut session = fixture.open();
  let report = session
    .discard(&[path("in-use.log"), path("free.log")], &bin.trash())
    .expect("discard");
  assert!(!report.complete());
  let Some(DiscardFailure::Trash(reason)) = &report.files()[0].failure else {
    panic!("the trash's refusal is told: {:?}", report.files()[0].failure);
  };
  assert!(reason.contains("os error 32"), "the system's own reason: {reason}");
  assert!(report.files()[1].failure.is_none(), "the other file is discarded");
  assert!(junk.exists() && !fixture.root().join("free.log").exists());
  assert_eq!(report.failed(), vec![path("in-use.log")]);
  assert_eq!(session.operation().phase(), LocalPhase::Review);
  drop(held);
  let again = session.discard(&report.failed(), &bin.trash()).expect("try again");
  assert!(again.complete());
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
}
