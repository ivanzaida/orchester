use std::path::PathBuf;

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_workspace::{runner::Runners, tasks::answering::PendingAnswers};

/// The answers sent from the window to `root`'s runner that its tasks were not read back with yet, whenever they
/// change; other projects' wake the stream without a render.
pub(super) fn answers(ctx: &mut Ctx, deps: (Runners, PathBuf)) -> PendingAnswers {
  let pending = ctx.stream(
    deps,
    |(runners, root), emitter: StreamEmitter<PendingAnswers, ()>| async move {
      let mut updates = runners.answers_of(&root);
      loop {
        let answers = updates.borrow_and_update().clone();
        if !emitter.emit(answers) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  pending.state().get().data.unwrap_or_default()
}
