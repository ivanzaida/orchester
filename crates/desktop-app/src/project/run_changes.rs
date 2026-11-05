//! One run's change count, for a screen that shows that run: its state and its transcript's lines, at most once per
//! [`TRANSCRIPT_PACE`], and nothing of the project's other runs (`Runners::run_changes_of`).

use std::path::Path;

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_core::id::RunId;

use super::feed::TRANSCRIPT_PACE;
use crate::services::Services;

/// How often run `run` of the project at `root` changed since the screen began to follow it; `0` without a run.
/// Reading it in a render re-renders it when the count moves.
pub fn run_changes(ctx: &mut Ctx, root: &Path, run: Option<RunId>) -> u64 {
  let deps = (Services::of(ctx).runners, root.to_path_buf(), run);
  let count = ctx.stream(
    deps,
    |(runners, root, run), emitter: StreamEmitter<u64, ()>| async move {
      let Some(run) = run else {
        return;
      };
      let mut updates = runners.run_changes_of(&root, run);
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) {
          return;
        }
        tokio::time::sleep(TRANSCRIPT_PACE).await;
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}
