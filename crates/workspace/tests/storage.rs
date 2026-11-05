//! The disk guard as Project settings › Runs › Storage and the MCP tools read it (ORC-422): the free space the
//! project keeps, whether the guard holds, and while it holds what could be cleaned — ended runs' worktrees first,
//! then idle warm workspaces, each the largest first — never a folder a run works in; and the Storage page's full
//! inventory, listed first and measured row by row. The real `orchester-runner` lists and sizes the folders
//! (`docs/architecture/remote-runners.md`, rule 5). Run with `cargo test -p orchester-workspace --features
//! test-support --test storage storage_`.

#[path = "support/runner_programs.rs"]
mod runner_programs;
mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  integration::WORKTREES_DIR,
  run::Run,
  store::{SettingsStore, WorkspaceStore, WorktreeStore},
  workspaces::{CleanableKind, DiskSpace, DiskSpaceError, DiskUsage, GIGABYTE, Holder, KeepFree, WorkspaceNumber},
};
use orchester_exec::disk_space::folder_size;
use orchester_ipc::{Rejection, SessionEnd, folders::ExecutorFolder};
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::Activity,
  runner::{ExecutorFolders, QUIET_STOP_TIMEOUT, RunRequestError, Runners},
  storage::{self, DiskGuard, KeptKind, KeptRow, Storage},
  tasks::changes::run_worktree,
  workspace::OpenProject,
};
use tokio::runtime::Handle;

use self::{
  runner_programs::programs,
  support::{Fixture, at},
};

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

/// The project of `fixture`'s runners, and its folders as the real runner reads them.
fn through_runner(fixture: &Fixture) -> (Runners, ExecutorFolders) {
  let project = OpenProject {
    project: fixture.db.project().clone(),
    repo_root: fixture.repo.path().to_path_buf(),
  };
  let runners = Runners::new(
    programs().runner.clone(),
    Handle::current(),
    Activity::new(),
    SessionEnd::Clean,
  );
  let folders = ExecutorFolders::new(runners.clone(), project);
  (runners, folders)
}

/// [`storage::read`] with `free` gigabytes free, off the async runtime as the app reads it.
async fn read(folders: &ExecutorFolders, root: &Path, free: u64) -> Storage {
  tokio::task::block_in_place(|| storage::read(root, &Disk(free), folders)).expect("read")
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_lists_what_could_be_cleaned_only_while_the_disk_guard_holds() {
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
  // A person's upkeep holds it: the runner gives back only what runs held when it starts.
  let idle = Holder::Run { run: small.id() };
  fixture
    .db
    .borrow_workspace(Holder::Upkeep, None, limit, at(7))
    .expect("borrow");
  fixture.db.borrow_workspace(idle, None, limit, at(8)).expect("borrow");
  fixture.db.return_workspace(idle, at(9)).expect("give back");
  let number = |n: u32| WorkspaceNumber::new(n).expect("above zero");
  fill(&number(1).path(&root), 80_000);
  fill(&number(2).path(&root), 20_000);
  let (runners, folders) = through_runner(&fixture);

  let calm = read(&folders, &root, 500).await;
  assert_eq!(calm.guard.keep_free, KeepFree::DEFAULT);
  assert!(!calm.guard.holds());
  assert!(
    calm.cleanable.is_empty(),
    "nothing is sized while the guard holds nothing back"
  );
  assert!(runners.status_of(&root).is_none(), "no runner is asked meanwhile");

  let low = read(&folders, &root, 32).await;
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
  let sized = |path: &Path| folder_size(path).ok();
  let measured: Vec<_> = low.cleanable.iter().map(|row| row.cleanable.bytes).collect();
  assert_eq!(
    measured,
    [
      sized(&run_worktree(&root, large.id())),
      sized(&run_worktree(&root, small.id())),
      sized(&number(2).path(&root)),
    ],
    "the runner's sizes are the folders' own"
  );
  assert_eq!(low.cleanable[2].cleanable.path, number(2).path(&root));
  assert_eq!(
    low.cleanable[0].run.as_ref().map(|(_, run)| *run),
    Some(large.number().get())
  );

  fixture
    .db
    .save_keep_free(KeepFree::from_gigabytes(20).expect("in range"))
    .expect("keep less");
  let kept_less = read(&folders, &root, 32).await;
  assert!(!kept_less.guard.holds(), "a changed keep-free applies at once");
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}

/// [`storage::inventory`] as the runner lists it, off the async runtime as the app reads it.
fn inventory(folders: &ExecutorFolders, root: &Path) -> Vec<KeptRow> {
  tokio::task::block_in_place(|| storage::inventory(root, folders)).expect("inventory")
}

/// Each of `rows` measured by the runner, one by one as the Storage page measures them.
fn measure(folders: &ExecutorFolders, rows: &mut [KeptRow]) {
  tokio::task::block_in_place(|| {
    for row in rows {
      storage::measure(row, folders);
    }
  });
}

/// A run of `task` that a person paused at `now`: its worktree stays in use, and the runner, which recovers a working
/// run without an agent as interrupted when it attaches, leaves it paused.
fn paused_run(fixture: &mut Fixture, task: orchester_core::id::TaskId, now: orchester_core::time::Timestamp) -> Run {
  let mut run = fixture.started_run(task, now);
  let events = run.pause(now).expect("pause");
  fixture.record(&run, &events);
  run
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_inventory_lists_all_folders_without_the_guard_and_sorts_measured_sizes() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  let task = fixture.task("Ship it", at(1));
  let mut ended = fixture.started_run(task, at(2));
  fixture.succeed(&mut ended, at(3));
  let working = paused_run(&mut fixture, task, at(4));
  fill(&run_worktree(&root, ended.id()), 1_000);
  fill(&run_worktree(&root, working.id()), 50_000);
  // A folder no run of the project owns is not listed.
  fill(&root.join(WORKTREES_DIR).join("someone-elses"), 7_000);
  let (runners, folders) = through_runner(&fixture);
  let mut rows = inventory(&folders, &root);
  assert_eq!(rows.len(), 2);
  assert!(rows.iter().all(|row| row.bytes.is_none()), "sizing is a separate pass");
  assert_eq!(
    rows
      .iter()
      .filter_map(storage::KeptRow::removable_run)
      .collect::<Vec<_>>(),
    [ended.id()]
  );
  measure(&folders, &mut rows);
  storage::largest_first(&mut rows);
  assert_eq!(rows[0].run, Some(working.id()));
  assert!(rows[0].in_use);
  assert_eq!(rows[0].bytes, Some(50_000));
  assert_eq!(rows[1].bytes, Some(1_000));
  assert_eq!(
    tokio::task::block_in_place(|| storage::measure_worktree(&folders, &run_worktree(&root, ended.id()))),
    Some(1_000),
    "the clean-up dialog's sizes are the runner's too"
  );
  assert_eq!(storage::paused_runs(&root).expect("paused"), 0);
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_inventory_lists_warm_workspaces_whose_folder_is_there_and_nothing_in_a_new_project() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  let task = fixture.task("Ship it", at(1));
  let run = fixture.started_run(task, at(2));
  let limit = NonZeroU32::new(3).expect("above zero");
  let holder = Holder::Run { run: run.id() };
  fixture
    .db
    .borrow_workspace(Holder::Upkeep, None, limit, at(3))
    .expect("borrow");
  fixture.db.borrow_workspace(holder, None, limit, at(4)).expect("borrow");
  fixture.db.return_workspace(holder, at(5)).expect("give back");
  let (runners, folders) = through_runner(&fixture);
  assert!(
    inventory(&folders, &root).is_empty(),
    "no worktrees folder and no workspace folder lists nothing"
  );

  let number = |n: u32| WorkspaceNumber::new(n).expect("above zero");
  fill(&number(2).path(&root), 20_000);
  let mut rows = inventory(&folders, &root);
  assert_eq!(rows.len(), 1, "workspace 1 has no folder");
  assert_eq!(rows[0].kind, KeptKind::Workspace(number(2)));
  assert_eq!(
    rows[0].run,
    Some(run.id()),
    "labelled with the run that worked in it last"
  );
  assert!(!rows[0].in_use);
  assert_eq!(
    rows[0].cleanable,
    Some(CleanableKind::IdleWorkspace { number: number(2) })
  );
  measure(&folders, &mut rows);
  assert_eq!(rows[0].bytes, Some(20_000));
  let outside = tokio::task::block_in_place(|| {
    folders.measure(ExecutorFolder::Worktree {
      name: "../workspaces".to_owned(),
    })
  });
  assert!(
    matches!(outside, Err(RunRequestError::Rejected(Rejection::Failed { .. }))),
    "a name that leads out of the worktrees folder is refused: {outside:?}"
  );
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}

#[test]
fn storage_save_keep_free_is_read_by_the_next_guard_check() {
  let fixture = Fixture::new();
  let root = fixture.repo.path();
  storage::save_keep_free(root, KeepFree::from_gigabytes(1).expect("minimum")).expect("save");
  let guard = || DiskGuard::read(&ProjectDb::open(root).expect("open"), root, &Disk(32)).expect("read");
  assert!(!guard().holds());
  storage::save_keep_free(root, KeepFree::from_gigabytes(1048576).expect("maximum")).expect("save");
  assert!(guard().holds());
  assert!(KeepFree::from_gigabytes(0).is_err());
  assert!(KeepFree::from_gigabytes(1048577).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_inventory_measures_step_worktrees_and_keeps_failed_steps_without_the_main_folder() {
  let mut fixture = Fixture::new();
  let root = fixture.repo.path().to_path_buf();
  let task = fixture.task("Ship it", at(1));
  let mut ended = fixture.started_run(task, at(2));
  fixture.succeed(&mut ended, at(3));
  let working = paused_run(&mut fixture, task, at(4));
  let step_path = |run: &Run| {
    root
      .join(WORKTREES_DIR)
      .join(run.current_attempt().expect("attempt").executions()[0].id().to_string())
  };
  let main = run_worktree(&root, ended.id());
  let step = step_path(&ended);
  let active_step = step_path(&working);
  fill(&main, 1_000);
  fill(&step, 50_000);
  fill(&active_step, 90_000);
  let (runners, folders) = through_runner(&fixture);
  let mut rows = inventory(&folders, &root);
  measure(&folders, &mut rows);
  assert_eq!(rows.len(), 3);
  assert_eq!(rows.iter().filter_map(|row| row.bytes).sum::<u64>(), 141_000);
  let active = rows.iter().find(|row| row.path == active_step).expect("active step");
  assert!(active.in_use);
  assert_eq!(active.removable_run(), None);
  let cleanable = read(&folders, &root, 32).await.cleanable;
  assert_eq!(cleanable.len(), 2);
  assert_eq!(
    cleanable.iter().filter_map(|row| row.cleanable.bytes).sum::<u64>(),
    51_000
  );

  fs::remove_dir_all(&main).expect("main removal succeeded");
  let left = orchester_core::integration::NotRemoved {
    path: step.to_string_lossy().replace('\\', "/"),
    reason: Some("held by a process".into()),
  };
  fixture
    .db
    .save_worktree_not_removed(ended.id(), &left, at(5))
    .expect("record failure");
  let rows = inventory(&folders, &root);
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
  let failed = rows
    .iter()
    .find(|row| row.path == step)
    .expect("failed step stays visible");
  assert_eq!(failed.run, Some(ended.id()));
  assert_eq!(failed.removable_run(), Some(ended.id()));
  assert_eq!(failed.removal.as_ref(), Some(&left));
  assert_eq!(rows.len(), 2);
}
