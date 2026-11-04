//! Inventory reads stream real measurement progress; no filesystem walk runs in render.
use std::path::PathBuf;

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, StreamEmitter},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  storage::{self, KeptRow},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
  pub rows: Vec<KeptRow>,
  pub measured: usize,
  pub finished: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading(pub Result<Inventory, Message>);
impl DevtoolsInspectable for Reading {}

pub fn follow(ctx: &mut Ctx, root: PathBuf, revision: (u64, u64, u64)) -> Option<Reading> {
  ctx
    .stream(
      (root, revision),
      |(root, _), emitter: StreamEmitter<Reading, ()>| async move {
        let errors = emitter.clone();
        let result = background::run(move || {
          let mut rows = storage::inventory(&root).map_err(|e| messages::store_error(&e))?;
          if !emitter.emit(Reading(Ok(Inventory {
            rows: rows.clone(),
            measured: 0,
            finished: rows.is_empty(),
          }))) {
            return Ok(());
          }
          let count = rows.len();
          for index in 0..count {
            storage::measure(&mut rows[index]);
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
          Ok::<_, Message>(())
        })
        .await
        .and_then(|r| r);
        if let Err(error) = result {
          errors.emit(Reading(Err(error)));
        }
      },
    )
    .state()
    .get()
    .data
}
