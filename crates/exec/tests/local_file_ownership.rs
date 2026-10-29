use std::{fs, path::PathBuf};

use orchester_core::integration::local::{LocalError, LocalPath};
use orchester_exec::local_git::{LocalGitError, digest, owned::OwnedFile};

fn directory() -> tempfile::TempDir {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  tempfile::tempdir_in(scratch).expect("private owned directory")
}
#[test]
fn local_integration_handle_cas_never_writes_a_substituted_final_entry() {
  let dir = directory();
  let original = dir.path().join("owned.txt");
  fs::write(&original, "reviewed").expect("original");
  let path = LocalPath::new("owned.txt").expect("path");
  let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("exists");
  let reviewed = digest(&file.read().expect("reviewed"));
  let moved = dir.path().join("old.txt");
  #[cfg(windows)]
  {
    assert!(fs::rename(&original, &moved).is_err());
    assert!(fs::write(&original, "foreign replacement").is_err());
    file
      .write(Some(&reviewed), b"explicit result")
      .expect("held object write");
    drop(file);
    assert_eq!(fs::read(&original).expect("result"), b"explicit result");
  }
  #[cfg(unix)]
  {
    fs::rename(&original, &moved).expect("private swap");
    fs::write(&original, "foreign replacement").expect("replacement");
    assert!(matches!(
      file.write(Some(&reviewed), b"must not overwrite"),
      Err(LocalGitError::State(LocalError::Stale))
    ));
    assert_eq!(fs::read(&original).expect("replacement"), b"foreign replacement");
    assert_eq!(fs::read(&moved).expect("original"), b"reviewed");
  }
}
#[test]
fn local_integration_handle_parent_chain_refuses_ancestor_substitution() {
  let dir = directory();
  let owned = dir.path().join("owned");
  let other = dir.path().join("private-other");
  fs::create_dir(&owned).expect("owned");
  fs::create_dir(&other).expect("private other");
  fs::write(owned.join("file.txt"), "reviewed").expect("original");
  fs::write(other.join("file.txt"), "private unrelated").expect("unrelated fixture");
  let path = LocalPath::new("owned/file.txt").expect("path");
  let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("exists");
  #[cfg(windows)]
  {
    assert!(fs::rename(&owned, dir.path().join("old")).is_err());
    assert_eq!(file.read().expect("owned bytes"), b"reviewed");
  }
  #[cfg(unix)]
  {
    fs::rename(&owned, dir.path().join("old")).expect("swap");
    std::os::unix::fs::symlink(&other, &owned).expect("private ancestor link");
    assert!(matches!(file.read(), Err(LocalGitError::State(LocalError::Stale))));
    assert!(
      file
        .write(Some(&digest(b"reviewed")), b"must not write outside")
        .is_err()
    );
  }
  assert_eq!(
    fs::read(other.join("file.txt")).expect("other fixture"),
    b"private unrelated"
  );
}
#[test]
fn local_integration_actual_handle_read_is_capped_and_originals_remain() {
  let dir = directory();
  let name = dir.path().join("large.bin");
  fs::write(&name, b"small before open").expect("original");
  let path = LocalPath::new("large.bin").expect("path");
  #[cfg(unix)]
  {
    let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("file");
    fs::OpenOptions::new()
      .write(true)
      .open(&name)
      .expect("fixture growth handle")
      .set_len(16 * 1024 * 1024 + 1)
      .expect("grow after open");
    assert!(matches!(
      file.read(),
      Err(LocalGitError::State(LocalError::Unsupported))
    ));
    assert!(file.remove(&digest(b"small before open")).is_err());
  }
  #[cfg(windows)]
  {
    {
      let file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("file");
      assert!(
        fs::OpenOptions::new().write(true).open(&name).is_err(),
        "concurrent growth denied by actual opened handle"
      );
      drop(file);
    }
    fs::OpenOptions::new()
      .write(true)
      .open(&name)
      .expect("fixture growth handle")
      .set_len(16 * 1024 * 1024 + 1)
      .expect("grow");
    let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("file");
    assert!(matches!(
      file.read(),
      Err(LocalGitError::State(LocalError::Unsupported))
    ));
    assert!(file.remove(&digest(b"small before open")).is_err());
  }
  assert_eq!(
    fs::metadata(&name).expect("original retained").len(),
    16 * 1024 * 1024 + 1
  );
}

#[test]
fn local_integration_hard_link_is_visibly_unsupported_before_read_write_or_remove() {
  let dir = directory();
  fs::write(dir.path().join("private-source"), b"private unrelated").expect("fixture");
  fs::hard_link(dir.path().join("private-source"), dir.path().join("link")).expect("private link");
  assert!(matches!(
    OwnedFile::open(dir.path(), &LocalPath::new("link").expect("path"), true),
    Err(LocalGitError::State(LocalError::Unsupported))
  ));
  assert_eq!(
    fs::read(dir.path().join("private-source")).expect("untouched"),
    b"private unrelated"
  );
}

#[cfg(windows)]
#[test]
fn local_integration_sqlite_parent_and_database_are_pinned_while_sidecars_stay_deletable() {
  let dir = directory();
  let metadata = dir.path().join(".orchester");
  fs::create_dir(&metadata).expect("private metadata");
  fs::write(metadata.join("orchester.db"), b"private database fixture").expect("private fixture");
  let guard = orchester_exec::local_git::owned::StoreOwnership::acquire(dir.path()).expect("pin opened objects");
  assert!(fs::rename(&metadata, dir.path().join("substituted")).is_err());
  assert!(fs::rename(metadata.join("orchester.db"), metadata.join("orchester.db.old")).is_err());
  assert!(fs::remove_file(metadata.join("orchester.db")).is_err());
  // SQLite in another process deletes its WAL and shared memory when its last connection closes.
  for name in ["orchester.db-wal", "orchester.db-shm", "orchester.db-journal"] {
    fs::remove_file(metadata.join(name)).expect("a pinned sidecar stays deletable");
  }
  assert_eq!(
    fs::read(metadata.join("orchester.db")).expect("actual held regular file"),
    b"private database fixture"
  );
  drop(guard);
  fs::rename(metadata.join("orchester.db"), metadata.join("released.db")).expect("pin releases normally");
}

/// Removal deletes only the held file: one swapped in under its name stays, and nothing is left behind.
#[test]
fn local_integration_conditional_removal_deletes_only_the_held_file() {
  let dir = directory();
  let path = LocalPath::new("owned.txt").expect("path");
  let original = dir.path().join("owned.txt");
  fs::write(&original, "reviewed").expect("original");
  let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("exists");
  let reviewed = digest(&file.read().expect("reviewed"));
  file.remove(&reviewed).expect("the held file is removed");
  assert!(!original.exists());

  fs::write(&original, "reviewed").expect("original again");
  let mut file = OwnedFile::open(dir.path(), &path, true).expect("open").expect("exists");
  let reviewed = digest(&file.read().expect("reviewed"));
  #[cfg(unix)]
  {
    fs::rename(&original, dir.path().join("old.txt")).expect("private swap");
    fs::write(&original, "foreign replacement").expect("replacement");
    assert!(matches!(
      file.remove(&reviewed),
      Err(LocalGitError::State(LocalError::Stale))
    ));
    assert_eq!(fs::read(&original).expect("replacement stays"), b"foreign replacement");
  }
  #[cfg(windows)]
  {
    assert!(fs::rename(&original, dir.path().join("old.txt")).is_err());
    file.remove(&reviewed).expect("the held file is removed");
  }
  let names: Vec<_> = fs::read_dir(dir.path())
    .expect("list")
    .map(|entry| entry.expect("entry").file_name())
    .collect();
  assert!(
    names
      .iter()
      .all(|name| !name.to_string_lossy().starts_with(".orchester-removing-")),
    "{names:?}"
  );
}

/// Unix cannot hold a name, so the store is checked instead: its folder and files are reached without a link, and
/// each is a regular file of one link.
#[cfg(unix)]
#[test]
fn local_integration_sqlite_parent_database_and_sidecars_are_checked_without_following_links() {
  use std::os::unix::fs::symlink;

  use orchester_exec::local_git::owned::StoreOwnership;
  let refused = |root: &std::path::Path| StoreOwnership::acquire(root).is_err();
  let store = |dir: &tempfile::TempDir| {
    let metadata = dir.path().join(".orchester");
    fs::create_dir(&metadata).expect("private metadata");
    fs::write(metadata.join("orchester.db"), b"private database fixture").expect("private fixture");
    metadata
  };

  let plain = directory();
  let metadata = store(&plain);
  fs::write(metadata.join("orchester.db-wal"), b"").expect("sidecar");
  StoreOwnership::acquire(plain.path()).expect("a plain store with a sidecar");

  let missing = directory();
  fs::create_dir(missing.path().join(".orchester")).expect("metadata without a database");
  assert!(refused(missing.path()), "a store without its database");

  let linked_folder = directory();
  let elsewhere = directory();
  store(&elsewhere);
  symlink(
    elsewhere.path().join(".orchester"),
    linked_folder.path().join(".orchester"),
  )
  .expect("link");
  assert!(refused(linked_folder.path()), "a linked .orchester");

  let linked_database = directory();
  let metadata = store(&linked_database);
  fs::rename(metadata.join("orchester.db"), metadata.join("real.db")).expect("move");
  symlink(metadata.join("real.db"), metadata.join("orchester.db")).expect("link");
  assert!(refused(linked_database.path()), "a linked database");

  let hard_linked = directory();
  let metadata = store(&hard_linked);
  fs::hard_link(metadata.join("orchester.db"), metadata.join("second.db")).expect("hard link");
  assert!(refused(hard_linked.path()), "a database of two links");

  let linked_sidecar = directory();
  let metadata = store(&linked_sidecar);
  symlink(elsewhere.path().join("outside"), metadata.join("orchester.db-wal")).expect("link");
  assert!(refused(linked_sidecar.path()), "a sidecar linked elsewhere");
}

#[cfg(windows)]
#[test]
fn local_integration_parent_chain_alone_prevents_rename_or_removal_before_git_or_store_opens() {
  let dir = directory();
  fs::create_dir_all(dir.path().join("parent/child")).expect("private directory");
  let guard = orchester_exec::local_git::owned::Directory::open(&dir.path().join("parent/child")).expect("held chain");
  assert!(fs::rename(dir.path().join("parent"), dir.path().join("replacement")).is_err());
  assert!(fs::remove_dir(dir.path().join("parent/child")).is_err());
  drop(guard);
  fs::rename(dir.path().join("parent"), dir.path().join("released")).expect("ownership releases normally");
}

#[cfg(windows)]
#[path = "support/local_reparse.rs"]
mod local_reparse;

#[cfg(windows)]
#[test]
fn local_integration_execution_root_anchors_refuse_in_place_reparse_substitution_of_scratch_and_git_directory() {
  use orchester_exec::local_git::owned::{Directory, ExecutionOwnership};
  let dir = directory();
  let target = dir.path().join("private-target");
  let control = dir.path().join("empty-control");
  fs::create_dir(&target).expect("private target");
  fs::write(target.join("private.txt"), b"unrelated private fixture").expect("private bytes");
  fs::create_dir(&control).expect("empty control");
  local_reparse::set_junction(&control, &target).expect("actual unguarded private junction control");
  fs::remove_dir(&control).expect("remove only our junction entry");
  let root = dir.path().join("repo");
  fs::create_dir_all(root.join(".git")).expect("private Git directory");
  // A Git directory is never empty, which is what keeps the held one from being converted in place.
  fs::write(root.join(".git/HEAD"), b"ref: refs/heads/main\n").expect("Git HEAD");
  fs::create_dir(root.join("scratch")).expect("scratch");
  let root_owner = Directory::open(&root).expect("root");
  let scratch = Directory::open(&root.join("scratch")).expect("scratch");
  let ownership = ExecutionOwnership::acquire(&root_owner, &scratch).expect("owned children");
  assert!(local_reparse::set_junction(&root.join("scratch"), &target).is_err());
  assert!(local_reparse::set_junction(&root.join(".git"), &target).is_err());
  ownership.ensure_current().expect("actual original chain remains");
  assert_eq!(
    fs::read(target.join("private.txt")).expect("unrelated fixture"),
    b"unrelated private fixture"
  );
}

#[cfg(windows)]
#[test]
fn local_integration_foreign_marker_is_refused_without_reading_linked_private_bytes() {
  use std::{os::windows::fs::OpenOptionsExt, process::Command};

  use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
  let dir = directory();
  let root = dir.path().join("repo");
  fs::create_dir(&root).expect("private repo");
  assert!(
    Command::new("git")
      .args(["init", "-b", "main"])
      .current_dir(&root)
      .output()
      .expect("git")
      .status
      .success()
  );
  fs::create_dir(root.join("scratch")).expect("scratch");
  let git = orchester_exec::local_git::LocalGit::new(&root, &root.join("scratch")).expect("owned Git");
  let private = dir.path().join("unrelated-private-fixture");
  fs::write(&private, b"private fixture must never be read as an operation marker").expect("fixture");
  fs::hard_link(&private, root.join(".git/index.lock")).expect("private linked marker");
  let _no_read = fs::OpenOptions::new()
    .write(true)
    .share_mode(FILE_SHARE_WRITE)
    .open(&private)
    .expect("deny reads of actual private object");
  assert_eq!(
    git.operation().expect("metadata-only foreign refusal"),
    Some("index.lock".into())
  );
}
