//! The MBZu9 inline decisions remain available when the file projection is refused.
use std::sync::atomic::Ordering;

use lurq::{
  app::{
    ctx::Ctx,
    events::{InputSource, input_source},
  },
  node::Element,
};

use super::{
  resolver::EditorState,
  work::{Action, Request, Work},
};
use crate::{
  navigation::Navigation,
  ui::{
    button::{ActionButton, ButtonKind},
    notice::{Notice, NoticeTone},
  },
};

pub(super) fn inline(
  ctx: &mut Ctx,
  work: &Work,
  state: &EditorState,
  request: Option<Request>,
  busy: bool,
  task: &super::TaskContext,
) -> Element {
  let save = save_pause(ctx, work, request, busy);
  let keep = state.leave.clone();
  let discarded = state.leave.clone();
  let navigation = Navigation::of(ctx);
  let editor = state.editor.clone();
  let work = work.clone();
  let refusal = Request {
    task: task.clone(),
    action: Action::Refused,
    stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
  };
  Notice::new("local-unsaved", NoticeTone::Warning, ctx.t("local_integration.unsaved"))
    .body(ctx.t("local_integration.unsavedBody"))
    .action(save)
    .action(
      ActionButton::new(
        "local-keep-editing",
        ButtonKind::Secondary,
        ctx.t("local_integration.keepEditing"),
      )
      .on_click(move || {
        if !busy {
          keep.keep_editing();
        }
      }),
    )
    .action(
      ActionButton::new(
        "local-discard-editor",
        ButtonKind::Danger,
        ctx.t("local_integration.discard"),
      )
      .on_click(move || {
        if busy {
          return;
        }
        if input_source() == InputSource::Synthetic {
          work.run_if_idle(refusal.clone());
          return;
        }
        editor.discard();
        discarded.leave(&navigation);
      }),
    )
    .into()
}

fn save_pause(ctx: &Ctx, work: &Work, request: Option<Request>, busy: bool) -> ActionButton {
  let save = ActionButton::new(
    "local-save-pause",
    ButtonKind::Secondary,
    ctx.t("local_integration.savePause"),
  );
  let Some(request) = request.filter(|_| !busy) else {
    return save.disabled();
  };
  let work = work.clone();
  save.on_click(move || {
    let mut request = request.clone();
    if input_source() == InputSource::Synthetic {
      request.action = Action::Refused;
    } else {
      request.stop.store(true, Ordering::Release);
    }
    work.run_if_idle(request);
  })
}
