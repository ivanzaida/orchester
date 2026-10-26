//! Fixed save candidates and the Connections page's person-only in-place question.
use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  agents::editor::{self, Action, Outcome, Request},
  background,
  messages::Message,
  tasks::outside::Outside,
};

use super::editor::Props;
use crate::{
  screens::connections::widen::{WidenConfirm, WidenLook, WidenProps},
  services::Services,
  ui::message::message_text,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
  request: Request,
  outcome: Outcome,
}
impl DevtoolsInspectable for Done {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending(pub Option<Request>);
impl DevtoolsInspectable for Pending {}
pub type Work = FutureAction<Request, Done, Message>;

pub fn work(ctx: &mut Ctx) -> Work {
  let root = ctx.props::<Props>().project.open.repo_root.clone();
  ctx.future_action(move |request:Request| {
    let root=root.clone();
    async move {
      background::run(move ||editor::apply(&root,&request).map(|outcome|Done {request,outcome})).await?
    }
  })
}
pub fn follow(ctx: &mut Ctx, work: &Work, pending: Signal<Pending>, busy: Signal<bool>) {
  let props = ctx.props::<Props>().clone();
  let changes = Services::of(ctx).changes;
  ctx.watch(&work.state(), move |state| {
    if state.is_pending() {
      return;
    }
    busy.set(false);
    if let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) {
      match done.outcome {
        Outcome::Saved(id) => {
          pending.set(Pending(None));
          props.nav.saved(id);
          // Every other screen that shows the profiles reads them again; this one reads them again once.
          props
            .profiles
            .record(&changes, props.project.open.project.id(), Outside::Profiles);
          props.revision.update(|revision| *revision += 1);
        }
        Outcome::Confirm(reasons) => pending.set(Pending(Some(Request {
          allowed: reasons,
          ..done.request
        }))),
      }
    }
  });
}
pub fn confirm(ctx: &mut Ctx, pending: &Signal<Pending>, busy: &Signal<bool>, work: &Work) -> Option<Element> {
  let request = pending.get().0?;
  let stale = matches!(request.action, Action::PublishStale);
  let mut reasons: Vec<Arc<str>> = request.allowed.iter().map(|reason| message_text(ctx, reason)).collect();
  if stale {
    reasons.insert(0, ctx.t("agents.keepWarning"));
  }
  let (keep, confirm, busy, work) = (pending.clone(), pending.clone(), busy.clone(), work.clone());
  // Editing a field dismisses the question; its fixed candidate can only be retried unchanged.
  let question = WidenProps {
    id: "agent-confirm".into(),
    look: WidenLook::Band,
    affirming: false,
    automation: if stale {
      crate::mcp::input_gate::Guarded::WideningQuestion
    } else if request
      .allowed
      .iter()
      .any(|reason| reason.key == "appMcp.widens.autoApprove")
    {
      crate::mcp::input_gate::Guarded::ApprovalWidening
    } else {
      crate::mcp::input_gate::Guarded::PermissionWidening
    },
    title: ctx.t(if stale { "agents.keepTitle" } else { "agents.widenTitle" }),
    reasons: reasons.into(),
    confirm: ctx.t(if stale { "agents.keepConfirm" } else { "agents.confirm" }),
    keep: ctx.t("agents.keepEditing"),
    on_keep: Arc::new(move || keep.set(Pending(None))),
    on_confirm: Arc::new(move || {
      if busy.get() {
        return;
      }
      busy.set(true);
      confirm.set(Pending(None));
      work.run(request.clone());
    }),
  };
  Some(ctx.mount_keyed::<WidenConfirm>("agent-confirm", question))
}
