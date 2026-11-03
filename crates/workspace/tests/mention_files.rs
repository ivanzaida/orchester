//! The repository's files as the @ menu offers them (ORC-316, `mentions::files`): what git tracks and does not ignore,
//! without deleted and hidden files, the run's changes first and then the most recently modified, filtered fuzzily by
//! path; and that filtering a repository of 50,000 files stays fast enough for the menu to follow typing.

use std::{
  fs,
  path::Path,
  process,
  time::{Duration, Instant, SystemTime},
};

use orchester_core::revision::Revision;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  files::FilePath,
  mentions::files::{FileIndex, Listed},
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
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

/// Writes its own path to `path` in `repo`, last modified `seconds` after a fixed time.
fn write(repo: &Path, path: &str, seconds: u64) {
  let file = repo.join(path);
  fs::create_dir_all(file.parent().expect("a folder")).expect("its folder");
  fs::write(&file, path).expect("write");
  touch(repo, path, seconds);
}

/// Makes `path` in `repo` last modified `seconds` after a fixed time.
fn touch(repo: &Path, path: &str, seconds: u64) {
  let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds);
  fs::File::options()
    .write(true)
    .open(repo.join(path))
    .and_then(|file| file.set_modified(at))
    .expect("set when it was modified");
}

/// A repository with `.gitignore` (ignoring `target/` and `.orchester/`), `README.md`, `src/sender.rs`,
/// `src/retry.rs` and `gone.rs` committed.
fn repository() -> (TempDir, Revision) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let repo = dir.path();
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "core.autocrlf", "false"]);
  fs::write(repo.join(".gitignore"), "target/\n.orchester/\n").expect(".gitignore");
  touch(repo, ".gitignore", 0);
  write(repo, "README.md", 10);
  write(repo, "src/sender.rs", 20);
  write(repo, "src/retry.rs", 30);
  write(repo, "gone.rs", 40);
  git(repo, &["add", "."]);
  git(repo, &["commit", "--quiet", "-m", "first"]);
  let head = Revision::new(&git(repo, &["rev-parse", "HEAD"])).expect("a commit");
  (dir, head)
}

fn paths(index: &FileIndex, query: &str) -> Vec<String> {
  index
    .offer(query)
    .shown
    .iter()
    .map(|file| file.path.to_string())
    .collect()
}

fn listed(path: &str, seconds: u64, changed: bool) -> Listed {
  Listed {
    path: FilePath::new(path).expect("a project path"),
    modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
    changed,
  }
}

#[test]
fn mention_files_list_tracked_and_untracked_files_never_ignored_ones() {
  let (dir, _) = repository();
  let repo = dir.path();
  write(repo, "target/debug/app.exe", 90);
  write(repo, ".orchester/project.db", 91);
  write(repo, "docs/new notes.md", 50);
  write(repo, "odd`name.md", 60);
  fs::remove_file(repo.join("gone.rs")).expect("delete");
  let programs = Programs::processes();
  let index = FileIndex::read(programs.runner(), repo, None).expect("listed");
  let mut all: Vec<String> = index.offer("").shown.iter().map(|file| file.path.to_string()).collect();
  all.truncate(3);
  assert_eq!(
    all,
    ["docs/new notes.md", "src/retry.rs", "src/sender.rs"],
    "the most recently modified first, an untracked one too"
  );
  assert_eq!(
    index.len(),
    5,
    "README.md, .gitignore, the two sources and the new notes"
  );
  for hidden in [
    "target/debug/app.exe",
    ".orchester/project.db",
    "gone.rs",
    "odd`name.md",
  ] {
    assert!(
      index
        .offer(hidden)
        .shown
        .iter()
        .all(|file| file.path.as_str() != hidden),
      "{hidden}: ignored, hidden, deleted or not writable as a mention"
    );
  }
  assert!(
    index.offer("").shown.iter().all(|file| !file.changed),
    "nothing is marked outside a run"
  );
}

#[test]
fn mention_files_of_a_run_lead_with_what_it_changed() {
  let (dir, base) = repository();
  let repo = dir.path();
  fs::write(repo.join("README.md"), "# Kontur, changed\n").expect("change it");
  git(repo, &["commit", "--quiet", "-am", "the run's checkpoint"]);
  touch(repo, "README.md", 5);
  fs::write(repo.join("src/sender.rs"), "fn send() {}\n").expect("change it");
  touch(repo, "src/sender.rs", 3);
  write(repo, "src/backoff.rs", 1);
  touch(repo, "src/retry.rs", 100);
  let programs = Programs::processes();
  let index = FileIndex::read(programs.runner(), repo, Some(&base)).expect("listed");
  let offered = index.offer("");
  let shown: Vec<(&str, bool)> = offered
    .shown
    .iter()
    .map(|file| (file.path.as_str(), file.changed))
    .collect();
  assert_eq!(
    shown,
    [("README.md", true), ("src/sender.rs", true), ("src/backoff.rs", true)],
    "committed, modified and untracked changes lead, the most recent first, before a newer file the run did not change"
  );
  assert_eq!(paths(&index, "retry"), ["src/retry.rs"]);
  assert!(!index.offer("retry").shown[0].changed);
}

#[test]
fn mention_files_filter_fuzzily_by_path_names_first() {
  let index = FileIndex::new(vec![
    listed("docs/webhooks/sending.md", 30, false),
    listed("crates/webhooks/src/sender.rs", 20, false),
    listed("crates/send/src/lib.rs", 50, false),
    listed("crates/webhooks/tests/sender_retry.rs", 10, false),
    listed("crates/http/src/client.rs", 40, false),
  ]);
  assert_eq!(
    paths(&index, "SEND"),
    [
      "docs/webhooks/sending.md",
      "crates/webhooks/src/sender.rs",
      "crates/webhooks/tests/sender_retry.rs",
      "crates/send/src/lib.rs",
    ],
    "names that hold it first, most recent first, then paths that do; ignoring case"
  );
  let sender = index.offer("send").shown[1].clone();
  assert_eq!(&sender.path.as_str()[sender.letters[0].clone()], "send");
  assert_eq!(sender.name_at(), "crates/webhooks/src/".len());

  let fuzzy = index.offer("cwsr");
  assert_eq!(fuzzy.total, 2, "the characters in order: {:?}", paths(&index, "cwsr"));
  let first = &fuzzy.shown[0];
  let letters: Vec<&str> = first
    .letters
    .iter()
    .map(|range| &first.path.as_str()[range.clone()])
    .collect();
  assert_eq!(first.path.as_str(), "crates/webhooks/src/sender.rs");
  assert_eq!(letters, ["c", "w", "s", "r"]);
  assert!(index.offer("payrol").shown.is_empty());
  let all = index.offer("");
  assert_eq!(
    (all.shown.len(), all.total),
    (3, 5),
    "three before typing, with how many there are"
  );
  assert!(all.shown.iter().all(|file| file.letters.is_empty()));
}

#[test]
fn mention_files_filter_fifty_thousand_files_while_typing() {
  let files: Vec<Listed> = (0..50_000u64)
    .map(|n| {
      let path = format!("crates/module_{}/src/part_{}/file_{n}.rs", n % 97, n % 13);
      listed(&path, n, n % 1000 == 0)
    })
    .collect();
  let built = Instant::now();
  let index = FileIndex::new(files);
  let build = built.elapsed();
  assert_eq!(index.len(), 50_000);
  let mut slowest = Duration::ZERO;
  for query in ["f", "fi", "file_4999", "m9p3", "mod_ul/zz", "zzzz", "FILE_123.RS"] {
    let started = Instant::now();
    let offer = index.offer(query);
    slowest = slowest.max(started.elapsed());
    assert!(offer.shown.len() <= 5, "only the rows shown are built");
  }
  assert_eq!(index.offer("file_49999.rs").shown.len(), 1);
  // A debug build; each keystroke filters once. Generous bounds that still catch a sort or an allocation per file.
  assert!(
    slowest < Duration::from_millis(150),
    "a keystroke filtered in {slowest:?}"
  );
  assert!(build < Duration::from_secs(2), "the index was built in {build:?}");
}
