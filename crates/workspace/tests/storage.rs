//! The disk guard as Project settings › Runs › Storage and the MCP tools read it (ORC-422): the free space the
//! project keeps, whether the guard holds, and while it holds what could be cleaned — ended runs' worktrees first,
//! then idle warm workspaces, each the largest first — never a folder a run works in.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  store::{SettingsStore, WorkspaceStore},
  workspaces::{CleanableKind, DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE, Holder, KeepFree, WorkspaceNumber},
};
use orchester_workspace::{storage, tasks::changes::run_worktree};

use self::support::{Fixture, at};

/// A disk with `free` gigabytes free.
struct Disk(u64);

impl DiskSpace for Disk {
  fn usage(&self, _folder: &Path) -> Result<DiskUsage, DiskSpaceError> {
    Ok(DiskUsage {
      free: self.0 * GIGABYTE,
      total: 2000 * GIGABYTE,
    })
  }
}

fn fill(folder: &Path, bytes: usize) {
  fs::create_dir_all(folder).expect("the folder");
  fs::write(folder.join("build.bin"), vec![0_u8; bytes]).expect("its build output");
}

#[test]
fn storage_lists_what_could_be_cleaned_only_while_the_disk_guard_holds() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  let task = fixture.task("Ship it", at(1));
  let mut small = fixture.started_run(task, at(2));
  fixture.succeed(&mut small, at(3));
  let mut large = fixture.started_run(task, at(4));
  fixture.succeed(&mut large, at(5));
  let working = fixture.started_run(task, at(6));
  fill(&run_worktree(&root, small.id()), 1_000);
  fill(&run_worktree(&root, large.id()), 50_000);
  fill(&run_worktree(&root, working.id()), 90_000);
  let limit = NonZeroU32::new(3).expect("above zero");
  let held = Holder::Run { run: working.id() };
  let idle = Holder::Run { run: small.id() };
  fixture.db.borrow_workspace(held, None, limit, at(7)).expect("borrow");
  fixture.db.borrow_workspace(idle, None, limit, at(8)).expect("borrow");
  fixture.db.return_workspace(idle, at(9)).expect("give back");
  let number = |n: u32| WorkspaceNumber::new(n).expect("above zero");
  fill(&number(1).path(&root), 80_000);
  fill(&number(2).path(&root), 20_000);

  let calm = storage::read(&root, &Disk(500)).expect("read");
  assert_eq!(calm.guard.keep_free, KeepFree::DEFAULT);
  assert!(!calm.guard.holds());
  assert!(
    calm.cleanable.is_empty(),
    "nothing is sized while the guard holds nothing back"
  );

  let low = storage::read(&root, &Disk(32)).expect("read");
  assert!(low.guard.holds(), "below the 50 GB a project keeps by default");
  let kinds: Vec<CleanableKind> = low.cleanable.iter().map(|row| row.cleanable.kind).collect();
  assert_eq!(
    kinds,
    [
      CleanableKind::EndedRunWorktree { run: large.id() },
      CleanableKind::EndedRunWorktree { run: small.id() },
      CleanableKind::IdleWorkspace { number: number(2) },
    ],
    "the working run's worktree and the held workspace are not offered"
  );
  assert!(low.cleanable[0].cleanable.bytes > low.cleanable[1].cleanable.bytes);
  assert_eq!(
    low.cleanable[0].run.as_ref().map(|(_, run)| *run),
    Some(large.number().get())
  );

  fixture
    .db
    .save_keep_free(KeepFree::from_gigabytes(20).expect("in range"))
    .expect("keep less");
  let kept_less = storage::read(&root, &Disk(32)).expect("read");
  assert!(!kept_less.guard.holds(), "a changed keep-free applies at once");
}
