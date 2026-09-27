//! Recording the drafts the editor's autosave stored (see [`crate::drafts`]).

use lurq::app::ctx::Ctx;
use orchester_core::resource::Pipeline;

use super::EditorHandle;
use crate::drafts::SaveAction;

/// Records what a finished save stored. It never starts a save: the running save already
/// stored every revision queued while it ran, and a failed one waits for the next edit.
pub(super) fn watch_saves(ctx: &mut Ctx, save: &SaveAction<Pipeline>, handle: &EditorHandle) {
  let state = handle.state.clone();
  ctx.watch(&save.state(), move |action| {
    if action.is_pending() {
      return;
    }
    if let Some(saved) = &action.data {
      state.update(|editor| {
        if let Some(editor) = editor {
          editor.saved(saved.revision, saved.at, saved.outcome);
        }
      });
    }
  });
}
