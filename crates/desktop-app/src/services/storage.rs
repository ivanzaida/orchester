//! Read-only disk sampling shared by Storage and the pinned status item.
use std::{path::PathBuf, time::Duration};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, StreamEmitter},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  storage::{self, DiskGuard},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
  pub guard: DiskGuard,
  pub paused: usize,
}
impl DevtoolsInspectable for Disk {}

#[derive(Clone, PartialEq)]
struct Reading(Result<Disk, Message>);
impl DevtoolsInspectable for Reading {}

pub fn follow(ctx: &mut Ctx, root: PathBuf, revision: u64) -> Option<Result<Disk, Message>> {
  ctx
    .stream(
      (root, revision),
      |(root, _), emitter: StreamEmitter<Reading, ()>| async move {
        let mut told = None;
        loop {
          let path = root.clone();
          let read = background::run(move || {
            let guard = storage::guard_local(&path).map_err(|e| messages::store_error(&e))?;
            let paused = storage::paused_runs(&path).map_err(|e| messages::store_error(&e))?;
            Ok::<_, Message>(Disk { guard, paused })
          })
          .await
          .and_then(|r| r);
          match read {
            Ok(disk) => {
              if told.as_ref() != Some(&disk) {
                if !emitter.emit(Reading(Ok(disk.clone()))) {
                  return;
                }
                told = Some(disk);
              }
            }
            Err(error) => {
              told = None;
              if !emitter.emit(Reading(Err(error))) {
                return;
              }
            }
          }
          tokio::time::sleep(Duration::from_secs(5)).await;
        }
      },
    )
    .state()
    .get()
    .data
    .map(|read| read.0)
}
