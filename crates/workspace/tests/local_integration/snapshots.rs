use std::fs;

use orchester_core::integration::local::LocalPath;
use orchester_workspace::tasks::local_integration::{Preview, ShownSnapshot, Snapshot, SnapshotSize, read};

use super::{
  privacy::Vault,
  support::{Fixture, git},
};

fn shown(path: &LocalPath, snapshot: Snapshot) -> ShownSnapshot {
  ShownSnapshot {
    path: path.clone(),
    snapshot,
  }
}

fn text(preview: &Preview) -> &str {
  match preview {
    Preview::Text(text) => text,
    other => panic!("a text diff, not {other:?}"),
  }
}

#[test]
fn local_snapshots_show_one_diff_each_and_report_their_sizes() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  fs::write(root.join("local.txt"), "base\nstaged\n").expect("staged");
  git(root, &["add", "local.txt"]);
  fs::write(root.join("local.txt"), "base\nstaged\nworking\nmore\n").expect("working");
  fs::write(root.join("trace.bin"), [7_u8, 0, 1, 2, 3]).expect("binary");
  fs::write(root.join("notes.md"), "new\n").expect("text");
  let local = LocalPath::new("local.txt").expect("path");
  let vault = Vault(None);

  let staged = read(root, fixture.run, Some(&shown(&local, Snapshot::Staged)), &vault).expect("read");
  assert!(text(&staged.diff).contains("+staged"));
  assert!(!text(&staged.diff).contains("+working"), "only HEAD → index");
  let unstaged = read(root, fixture.run, Some(&shown(&local, Snapshot::Unstaged)), &vault).expect("read");
  assert!(text(&unstaged.diff).contains("+working"));
  assert!(!text(&unstaged.diff).contains("+staged"), "only index → working copy");

  let stats = &staged.stats;
  assert_eq!(
    stats.of(&local, Snapshot::Staged),
    Some(SnapshotSize::Lines { added: 1, removed: 0 })
  );
  assert_eq!(
    stats.of(&local, Snapshot::Unstaged),
    Some(SnapshotSize::Lines { added: 2, removed: 0 })
  );
  let binary = LocalPath::new("trace.bin").expect("path");
  assert_eq!(
    stats.of(&binary, Snapshot::Untracked),
    Some(SnapshotSize::Binary(Some(5)))
  );
  let notes = LocalPath::new("notes.md").expect("path");
  assert_eq!(stats.of(&notes, Snapshot::Untracked), Some(SnapshotSize::NewText));
  assert_eq!(stats.of(&notes, Snapshot::Staged), None);
  let untracked = read(root, fixture.run, Some(&shown(&notes, Snapshot::Untracked)), &vault).expect("read");
  assert_eq!(untracked.diff, Preview::Text("new\n".to_owned()));
}

#[test]
fn local_snapshots_read_a_large_ignored_folder_as_one_bounded_entry() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  fs::write(root.join(".git/info/exclude"), "build/\n").expect("exclude rule");
  fs::create_dir_all(root.join("build")).expect("ignored folder");
  fs::write(root.join("build/first.o"), b"object").expect("first ignored file");
  let scratch = orchester_exec::local_git::prepare_scratch(root, &[".orchester", "snapshot-test"]).expect("scratch");
  let main = orchester_exec::local_git::LocalGit::new(root, &scratch).expect("main");
  let before = main.snapshot().expect("snapshot");
  for folder in 0..40 {
    let folder = root.join(format!("build/deps/{folder}"));
    fs::create_dir_all(&folder).expect("nested ignored folder");
    for file in 0..100 {
      fs::write(folder.join(format!("artifact-{file}.rlib")), b"build output").expect("ignored file");
    }
  }
  let started = std::time::Instant::now();
  let after = main.snapshot().expect("snapshot with thousands of ignored files");
  let elapsed = started.elapsed();
  assert_eq!(
    after, before,
    "files inside an ignored folder are not part of the checkout's identity"
  );
  assert!(
    elapsed < std::time::Duration::from_secs(20),
    "bounded read: {elapsed:?}"
  );
  let view = read(root, fixture.run, None, &Vault(None)).expect("the review reads the checkout");
  assert!(
    view.checkout.files.is_empty(),
    "ignored content is never listed for commit or preservation"
  );
}

/// Every entry under `root` with its size, folders as size 0.
fn entries(root: &std::path::Path) -> Vec<(std::path::PathBuf, u64)> {
  let mut result = Vec::new();
  let mut pending = vec![root.to_owned()];
  while let Some(directory) = pending.pop() {
    for entry in fs::read_dir(directory).expect("entries") {
      let entry = entry.expect("entry");
      let metadata = entry.metadata().expect("metadata");
      let relative = entry.path().strip_prefix(root).expect("relative").to_owned();
      if metadata.is_dir() {
        pending.push(entry.path());
        result.push((relative, 0));
      } else {
        result.push((relative, metadata.len()));
      }
    }
  }
  result.sort();
  result
}

#[test]
fn local_snapshots_and_review_reads_leave_git_unchanged() {
  let fixture = Fixture::new(false);
  let root = fixture.root();
  fs::write(root.join("local.txt"), "base\nstaged\n").expect("staged");
  git(root, &["add", "local.txt"]);
  fs::write(root.join("local.txt"), "base\nstaged\nworking\n").expect("working");
  fs::write(root.join("notes.md"), "new\n").expect("untracked");
  let before = entries(&root.join(".git"));
  let scratch = orchester_exec::local_git::prepare_scratch(root, &[".orchester", "snapshot-test"]).expect("scratch");
  let main = orchester_exec::local_git::LocalGit::new(root, &scratch).expect("main");
  main.snapshot().expect("snapshot");
  drop(main);
  let local = LocalPath::new("local.txt").expect("path");
  read(
    root,
    fixture.run,
    Some(&shown(&local, Snapshot::Unstaged)),
    &Vault(None),
  )
  .expect("review read");
  orchester_workspace::tasks::local_integration::fingerprint(root, fixture.run).expect("poll");
  assert_eq!(
    entries(&root.join(".git")),
    before,
    "reading local changes writes nothing into .git"
  );
}
