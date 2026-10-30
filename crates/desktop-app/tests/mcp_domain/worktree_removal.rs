//! A run whose worktree removal left its folder on disk (ORC-418): `get_run` answers its worktree `not_removed`, with
//! the folder and why in `worktree_not_removed`, never `removed`; once the folder is gone it reads `removed`.

use std::fs;

use orchester_core::{
  integration::NotRemoved,
  run::Run,
  store::{RunStore, TaskStore, WorktreeStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Seeded, Served};

/// The seeded run ready for review.
fn ready_run(seeded: &Seeded) -> Run {
  let db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Show the signed-in account")
    .expect("the seeded task ready for review");
  db.runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .find_map(Result::ok)
    .expect("its run")
}

#[test]
fn domain_worktree_removal_that_left_the_folder_reads_not_removed_in_get_run() {
  let (mut served, seeded) = Served::seeded("tasks");
  let run = ready_run(&seeded);
  let folder = seeded.repo.join(".orchester/worktrees").join(run.id().to_string());
  fs::create_dir_all(folder.join("target/debug")).expect("the folder that stayed");
  fs::write(folder.join("target/debug/app.bin"), "built\n").expect("its build output");
  let path = folder.to_string_lossy().replace('\\', "/");
  let reason = "Access is denied. (os error 5)";
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let at = Timestamp::now();
  db.save_worktree_removal(run.id(), at).expect("record the removal");
  let left = NotRemoved {
    path: path.clone(),
    reason: Some(reason.to_owned()),
  };
  db.save_worktree_not_removed(run.id(), &left, at)
    .expect("record what stayed");
  let in_run = json!({ "project": "Kontur", "run": run.id().to_string() });

  let read = served.ok("get_run", &in_run);
  assert_eq!(read["worktree"], "not_removed", "{read}");
  assert_eq!(
    read["worktree_not_removed"],
    json!({ "path": path, "reason": reason }),
    "{read}"
  );

  fs::remove_dir_all(&folder).expect("the folder goes");
  let read = served.ok("get_run", &in_run);
  assert_eq!(read["worktree"], "removed", "{read}");
  assert_eq!(read["worktree_not_removed"], Value::Null, "{read}");
}
