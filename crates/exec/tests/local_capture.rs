//! The verified checkout read hashes only what may have changed since a verified read proved it, and still sees every
//! local edit.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::{Duration, Instant},
};

use orchester_core::integration::local::{Checkout, LocalError};
use orchester_exec::local_git::{LocalGit, LocalGitError};

/// Longer than the time a file must have been left alone for its stamp to prove it.
const SETTLE: Duration = Duration::from_millis(3_500);

struct Repo {
  git: LocalGit,
  dir: tempfile::TempDir,
}

impl Repo {
  /// A checkout whose tracked files were left alone long enough, then read verified once, so their stamps are
  /// proven.
  fn proven() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-280-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("capture-")
      .tempdir_in(scratch)
      .expect("repo");
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "test@example.invalid"]);
    git(dir.path(), &["config", "user.name", "Capture test"]);
    git(dir.path(), &["config", "core.autocrlf", "false"]);
    fs::write(dir.path().join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(dir.path().join("proven.txt"), "original\n").expect("file");
    fs::write(dir.path().join("other.txt"), "other\n").expect("file");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "base"]);
    fs::create_dir_all(dir.path().join(".orchester/scratch")).expect("scratch");
    let local = LocalGit::new(dir.path(), &dir.path().join(".orchester/scratch")).expect("git");
    std::thread::sleep(SETTLE);
    let repo = Self { git: local, dir };
    assert!(repo.git.verified_snapshot().expect("first verified read").clean());
    let stamps = fs::read(repo.root().join(".orchester/local-integration/verified-stamps")).expect("stamps kept");
    assert!(stamps.windows(b"proven.txt".len()).any(|part| part == b"proven.txt"));
    repo
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn changed(&self, checkout: &Checkout) -> Vec<(String, bool)> {
    checkout
      .files
      .iter()
      .map(|file| (file.path.as_str().to_owned(), file.unstaged))
      .collect()
  }
}

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .args(args)
    .current_dir(root)
    .output()
    .expect("git starts");
  assert!(output.status.success(), "Git command failed: {args:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

#[test]
fn local_capture_proven_checkout_reads_the_same_as_a_fully_hashed_one() {
  let repo = Repo::proven();
  let first = repo.git.verified_snapshot().expect("proven read");
  fs::remove_file(repo.root().join(".orchester/local-integration/verified-stamps")).expect("forget proofs");
  assert_eq!(repo.git.verified_snapshot().expect("fully hashed read"), first);
  assert!(first.clean());
}

#[test]
fn local_capture_same_size_edit_with_restored_timestamp_is_seen_after_its_file_was_proven() {
  let repo = Repo::proven();
  let reviewed = repo.git.verified_snapshot().expect("reviewed");
  let filename = repo.root().join("proven.txt");
  let modified = fs::metadata(&filename).expect("metadata").modified().expect("mtime");
  fs::write(&filename, b"external\n").expect("same sized write");
  fs::File::options()
    .write(true)
    .open(&filename)
    .expect("file")
    .set_modified(modified)
    .expect("restore timestamp");
  assert_eq!(
    fs::metadata(&filename).expect("metadata").len(),
    b"original\n".len() as u64
  );
  let current = repo.git.verified_snapshot().expect("verified");
  assert_eq!(repo.changed(&current), [("proven.txt".to_owned(), true)]);
  assert!(matches!(
    repo.git.ensure(&reviewed),
    Err(LocalGitError::State(LocalError::Stale))
  ));
}

#[test]
fn local_capture_proven_file_is_hashed_again_when_its_index_entry_changed() {
  let repo = Repo::proven();
  let object = git(repo.root(), &["hash-object", "-w", "--stdin", "--path=other.txt"]);
  git(
    repo.root(),
    &["update-index", "--cacheinfo", &format!("100644,{object},proven.txt")],
  );
  let current = repo.git.verified_snapshot().expect("verified");
  assert_eq!(repo.changed(&current), [("proven.txt".to_owned(), true)]);
}

#[test]
fn local_capture_removed_proven_file_is_seen() {
  let repo = Repo::proven();
  fs::remove_file(repo.root().join("proven.txt")).expect("remove");
  let current = repo.git.verified_snapshot().expect("verified");
  assert_eq!(repo.changed(&current), [("proven.txt".to_owned(), true)]);
}

#[cfg(unix)]
#[test]
fn local_capture_executable_bit_set_on_a_proven_file_is_seen() {
  use std::os::unix::fs::PermissionsExt;
  let repo = Repo::proven();
  let filename = repo.root().join("proven.txt");
  fs::set_permissions(&filename, fs::Permissions::from_mode(0o755)).expect("mode");
  let current = repo.git.verified_snapshot().expect("verified");
  assert_eq!(repo.changed(&current), [("proven.txt".to_owned(), true)]);
}

/// Times the checkout reads on the checkout this repository is built in, the person's main one included:
/// `cargo test -p orchester-exec --test local_capture local_capture_timing -- --ignored --nocapture`. It forgets the
/// checkout's proven stamps first, so the first verified read hashes every tracked file and proves them again.
#[test]
#[ignore = "measures the checkout it runs in"]
#[allow(clippy::print_stderr, reason = "the timings ORC-280 asks to report")]
fn local_capture_timing_of_this_checkout() {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../..")
    .canonicalize()
    .expect("root");
  let scratch = root.join(".tmp/orc-280-timing");
  fs::create_dir_all(&scratch).expect("scratch");
  let stamps = root.join(".orchester/local-integration/verified-stamps");
  if stamps.exists() {
    fs::remove_file(&stamps).expect("forget proofs");
  }
  let local = LocalGit::new(&root, &scratch).expect("git");
  let timed = |label: &str, read: &dyn Fn() -> Checkout| {
    let start = Instant::now();
    read();
    eprintln!("{label}: {:.3} s", start.elapsed().as_secs_f64());
  };
  timed("cached read", &|| local.snapshot().expect("cached"));
  timed("verified read, every file hashed", &|| {
    local.verified_snapshot().expect("verified")
  });
  timed("verified read, proven files kept", &|| {
    local.verified_snapshot().expect("verified")
  });
}
