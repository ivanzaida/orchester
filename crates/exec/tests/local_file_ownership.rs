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
fn local_integration_sqlite_parent_database_and_sidecars_are_pinned_before_the_vfs_opens() {
  let dir = directory();
  let metadata = dir.path().join(".orchester");
  fs::create_dir(&metadata).expect("private metadata");
  fs::write(metadata.join("orchester.db"), b"private database fixture").expect("private fixture");
  let guard = orchester_exec::local_git::owned::StoreOwnership::acquire(dir.path()).expect("pin opened objects");
  assert!(fs::rename(&metadata, dir.path().join("substituted")).is_err());
  for name in [
    "orchester.db",
    "orchester.db-wal",
    "orchester.db-shm",
    "orchester.db-journal",
  ] {
    assert!(fs::rename(metadata.join(name), metadata.join(format!("{name}.old"))).is_err());
    assert!(fs::remove_file(metadata.join(name)).is_err());
  }
  assert_eq!(
    fs::read(metadata.join("orchester.db")).expect("actual held regular file"),
    b"private database fixture"
  );
  drop(guard);
  fs::rename(metadata.join("orchester.db"), metadata.join("released.db")).expect("pin releases normally");
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
