//! A worktree folder git forgot is read only as far as its first entries, in name order, so a
//! folder full of build output does not slow every read of what runs left; an entry that cannot be
//! read ends its listing with why instead of failing the read. Either way removing it loses files,
//! so it goes only once a person agreed to the listing as read.

use std::{fs, path::Path};

use orchester_core::integration::{FOLDER_ENTRIES_READ, FolderListing, KeepReason, clean_up, leftovers};

use crate::{
  cleanup::repo_with_leftovers,
  support::{Ending, Processes, Repo, git_program},
};

/// The run's own worktree, forgotten by git with its folder left behind.
fn forget_own(repo: &Repo, run: &orchester_core::run::Run) -> std::path::PathBuf {
  let folder = repo.root.join(".orchester/worktrees").join(run.id().to_string());
  repo.git(&["worktree", "remove", "--force", folder.to_str().expect("a UTF-8 path")]);
  fs::create_dir_all(&folder).expect("a folder left behind");
  folder
}

#[test]
fn forgotten_folder_listing_stops_after_its_first_entries() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let folder = forget_own(&repo, &run);
  let target = folder.join("target");
  fs::create_dir_all(&target).expect("build output");
  for index in 0..FOLDER_ENTRIES_READ + 50 {
    fs::write(target.join(format!("{index:04}.o")), "object\n").expect("write");
  }

  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read");
  let forgotten = left
    .worktrees
    .iter()
    .find(|worktree| worktree.forgotten)
    .expect("the folder is listed");
  assert_eq!(forgotten.listing, FolderListing::Cut);
  // The folder `target` is one of the entries read.
  assert_eq!(forgotten.uncommitted.len(), FOLDER_ENTRIES_READ - 1);
  assert_eq!(forgotten.uncommitted[0], "target/0000.o", "the first in name order");
  assert!(left.loses_files() && left.listing_cut() && !left.listing_unreadable());
  let again = leftovers(&Processes, &git_program(), &repo.root, &run).expect("read again");
  assert_eq!(again, left, "a folder that did not change reads the same");

  let kept = clean_up(&Processes, &git_program(), &repo.root, &run, None).expect("cleared");
  assert!(
    kept
      .kept_worktrees
      .iter()
      .any(|(worktree, reason)| worktree.forgotten && matches!(reason, KeepReason::Uncommitted(_))),
    "nobody agreed to lose its files"
  );
  assert!(folder.is_dir());

  let cleared = clean_up(&Processes, &git_program(), &repo.root, &run, Some(&left)).expect("cleared");
  assert!(cleared.removed_run_worktree());
  assert!(!folder.exists());
}

#[test]
fn forgotten_folder_with_an_unreadable_entry_is_listed_with_why() {
  let (repo, run) = repo_with_leftovers(Ending::Interrupted);
  let folder = forget_own(&repo, &run);
  fs::write(folder.join("a.txt"), "read\n").expect("write");
  fs::write(folder.join("z.txt"), "after\n").expect("write");
  let locked = folder.join("locked");
  fs::create_dir_all(&locked).expect("a folder");
  fs::write(locked.join("held.bin"), "held\n").expect("write");
  let guard = Unlistable::new(&locked);

  let left = leftovers(&Processes, &git_program(), &repo.root, &run).expect("the read does not fail");
  let forgotten = left
    .worktrees
    .iter()
    .find(|worktree| worktree.forgotten)
    .expect("the folder is listed");
  let FolderListing::Unreadable { path, detail } = &forgotten.listing else {
    panic!(
      "the listing says which entry could not be read: {:?}",
      forgotten.listing
    );
  };
  assert!(path.ends_with("/locked"), "{path}");
  assert!(!detail.is_empty());
  assert_eq!(forgotten.uncommitted, ["a.txt"], "the files read before it");
  assert!(forgotten.loses_files());
  assert!(
    left.listing_unreadable() && !left.listing_cut(),
    "it is not said to hold more"
  );
  assert_eq!(
    left.worktrees.iter().filter(|worktree| !worktree.forgotten).count(),
    1,
    "the step's worktree is read as usual"
  );
  drop(guard);
}

/// A folder whose entries cannot be listed until this is dropped.
struct Unlistable<'a>(&'a Path);

impl<'a> Unlistable<'a> {
  fn new(dir: &'a Path) -> Self {
    deny_listing(dir, true);
    Self(dir)
  }
}

impl Drop for Unlistable<'_> {
  fn drop(&mut self) {
    deny_listing(self.0, false);
  }
}

#[cfg(windows)]
fn deny_listing(dir: &Path, deny: bool) {
  // Everyone, by its well-known SID, may not list the folder.
  let args: &[&str] = if deny {
    &["/deny", "*S-1-1-0:(RD)"]
  } else {
    &["/remove:d", "*S-1-1-0"]
  };
  let status = std::process::Command::new("icacls")
    .arg(dir)
    .args(args)
    .stdout(std::process::Stdio::null())
    .status()
    .expect("icacls runs");
  assert!(status.success(), "icacls changed the folder's access");
}

#[cfg(unix)]
fn deny_listing(dir: &Path, deny: bool) {
  use std::os::unix::fs::PermissionsExt;
  let mode = if deny { 0o000 } else { 0o755 };
  fs::set_permissions(dir, fs::Permissions::from_mode(mode)).expect("the folder's mode changes");
}
