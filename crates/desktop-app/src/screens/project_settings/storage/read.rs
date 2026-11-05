//! Inventory reads stream real measurement progress; no filesystem walk runs in render. The project's runner lists
//! the folders and measures each row (`docs/architecture/remote-runners.md`, rule 5).
use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, StreamEmitter},
};
use orchester_workspace::{
  background,
  messages::Message,
  runner::ExecutorFolders,
  storage::{self, KeptRow},
  workspace::OpenProject,
};

use crate::services::Services;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
  pub rows: Vec<KeptRow>,
  pub measured: usize,
  pub finished: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading(pub Result<Inventory, Message>);
impl DevtoolsInspectable for Reading {}

pub fn follow(ctx: &mut Ctx, open: &OpenProject, revision: (u64, u64, u64)) -> Option<Reading> {
  let folders = ExecutorFolders::new(Services::of(ctx).runners, open.clone());
  ctx
    .stream(
      (open.repo_root.clone(), revision),
      move |(root, _): (PathBuf, _), emitter: StreamEmitter<Reading, ()>| {
        let folders = folders.clone();
        async move {
          let errors = emitter.clone();
          let result = background::run(move || measure_all(&root, &folders, &emitter))
            .await
            .and_then(|r| r);
          if let Err(error) = result {
            errors.emit(Reading(Err(error)));
          }
        }
      },
    )
    .state()
    .get()
    .data
}

/// Lists the rows, then measures them one by one, showing each step; stops once nobody follows.
fn measure_all(
  root: &std::path::Path,
  folders: &ExecutorFolders,
  emitter: &StreamEmitter<Reading, ()>,
) -> Result<(), Message> {
  let mut rows = storage::inventory(root, folders).map_err(|e| e.message())?;
  if !emitter.emit(Reading(Ok(Inventory {
    rows: rows.clone(),
    measured: 0,
    finished: rows.is_empty(),
  }))) {
    return Ok(());
  }
  let count = rows.len();
  for index in 0..count {
    storage::measure(&mut rows[index], folders);
    let mut shown = rows.clone();
    storage::largest_first(&mut shown);
    if !emitter.emit(Reading(Ok(Inventory {
      rows: shown,
      measured: index + 1,
      finished: index + 1 == count,
    }))) {
      return Ok(());
    }
  }
  Ok(())
}
