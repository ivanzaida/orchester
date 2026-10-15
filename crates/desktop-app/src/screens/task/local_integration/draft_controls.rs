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
  (busy, safe_discard): (bool, bool),
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
    input: None,
    generation: None,
  };
  let gate: crate::mcp::input_gate::InputGate = ctx.use_context().unwrap_or_default();
  let keep_gate = gate.clone();
  let keep_work = work.clone();
  let mut keep_refusal = refusal.clone();
  keep_refusal.action = Action::AuthorityRefused;
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
          if keep_gate.local_input(input_source()).allowed() {
            keep.keep_editing();
          } else {
            keep_work.run_if_idle(keep_refusal.clone());
          }
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
        if !gate.local_input(input_source()).allowed() || (input_source() == InputSource::Synthetic && !safe_discard) {
          let mut refusal = refusal.clone();
          refusal.initiate(&gate);
          work.run_if_idle(refusal);
          return;
        }
        editor.discard();
        discarded.leave(&navigation);
      }),
    )
    .into()
}

fn save_pause(ctx: &mut Ctx, work: &Work, request: Option<Request>, busy: bool) -> ActionButton {
  let save = ActionButton::new(
    "local-save-pause",
    ButtonKind::Secondary,
    ctx.t("local_integration.savePause"),
  );
  let Some(request) = request.filter(|_| !busy) else {
    return save.disabled();
  };
  let work = work.clone();
  let gate: crate::mcp::input_gate::InputGate = ctx.use_context().unwrap_or_default();
  save.on_click(move || {
    let mut request = request.clone();
    request.initiate(&gate);
    request.stop.store(true, Ordering::Release);
    work.run_if_idle(request);
  })
}
