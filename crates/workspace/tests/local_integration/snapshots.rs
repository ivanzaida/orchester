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
