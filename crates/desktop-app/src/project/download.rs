//! A shared project's first download after a join, as its feed follows it (ORC-282): how the download of its tasks
//! from its team server stands (`Servers::download_state`), read again whenever a shared project's cache or a
//! download changed. The Tasks list says so while the project has no task yet.

use std::path::{Path, PathBuf};

use lurq::app::ctx::{Ctx, StreamEmitter};
use orchester_workspace::sharing::{Servers, project::FirstDownload};

/// How the first download of the project at `root` stands; `None` while none is under way or failed. Other projects'
/// changes wake the stream without a render.
pub(super) fn state(ctx: &mut Ctx, (servers, root): (&Servers, &Path)) -> Option<FirstDownload> {
  let deps = (servers.clone(), root.to_path_buf());
  let state = ctx.stream(
    deps,
    |(servers, root): (Servers, PathBuf), emitter: StreamEmitter<Option<FirstDownload>, ()>| async move {
      let mut updates = servers.subscribe_shared();
      let mut last = None;
      loop {
        updates.borrow_and_update();
        let state = servers.download_state(&root);
        if last.as_ref() != Some(&state) {
          if !emitter.emit(state.clone()) {
            return;
          }
          last = Some(state);
        }
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  state.state().get().data.flatten()
}
