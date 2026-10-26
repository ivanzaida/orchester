//! A profile page's questions for the person, asked in place: what a change widens, at the section of the field that
//! widens it, before it is saved (*Allow these changes to the agent profile?* in 01c), and publishing a stale draft
//! over the latest version, in place of the draft bar (*Publish this draft over the published version?*, 01e). Each
//! is a fixed candidate: editing a field takes the question away, and only the person's own input answers it.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::agents::editor::Action;

use super::{
  permissions::Section,
  work::{Allowed, Pending, Work},
};
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{WidenConfirm, WidenProps},
  ui::message::message_text,
};

/// The signals a question's answer changes.
#[derive(Clone)]
pub struct Asking {
  pub pending: Signal<Pending>,
  pub allowed: Signal<Allowed>,
  pub work: Work,
}

impl Asking {
  /// The person allowed `request`: it runs, unless a write is under way.
  fn answer(&self, request: &orchester_workspace::agents::editor::Request) {
    if self.work.run_if_idle(request.clone()) {
      self.pending.set(Pending(None));
    }
  }
}

/// What the pending candidate widens, at its section; `None` without one, or for a stale publication.
pub fn widening(ctx: &mut Ctx, asking: &Asking) -> Option<(Section, Element)> {
  let asked = asking.pending.get().0?;
  if asked.request.action == Action::PublishStale {
    return None;
  }
  let section = asked.reasons.first().map_or(Section::Connection, Section::of);
  let reasons: Vec<Arc<str>> = asked.reasons.iter().map(|reason| message_text(ctx, reason)).collect();
  let automation = if asked
    .reasons
    .iter()
    .any(|reason| reason.key == "appMcp.widens.autoApprove")
  {
    Guarded::ApprovalWidening
  } else {
    Guarded::PermissionWidening
  };
  let (keep, confirm) = (asking.pending.clone(), asking.clone());
  let question = WidenProps {
    id: "agent-confirm".into(),
    card: true,
    affirming: false,
    automation,
    title: ctx.t("agents.widenTitle"),
    reasons: reasons.into(),
    confirm: ctx.t("agents.confirm"),
    keep: ctx.t("agents.keepEditing"),
    on_keep: Arc::new(move || keep.set(Pending(None))),
    on_confirm: Arc::new(move || {
      confirm.allowed.update(|allowed| {
        for reason in &asked.reasons {
          if !allowed.0.contains(reason) {
            allowed.0.push(reason.clone());
          }
        }
      });
      confirm.answer(&asked.request);
    }),
  };
  Some((section, ctx.mount_keyed::<WidenConfirm>("agent-confirm", question)))
}

/// Publishing a stale draft over the latest version, asked in place of the draft bar.
pub fn stale(ctx: &mut Ctx, asking: &Asking) -> Option<Element> {
  let asked = asking.pending.get().0?;
  if asked.request.action != Action::PublishStale {
    return None;
  }
  let (keep, confirm) = (asking.pending.clone(), asking.clone());
  let question = WidenProps {
    id: "agent-confirm".into(),
    card: true,
    affirming: false,
    automation: Guarded::WideningQuestion,
    title: ctx.t("agents.keepTitle"),
    reasons: vec![ctx.t("agents.keepWarning")].into(),
    confirm: ctx.t("agents.keepConfirm"),
    keep: ctx.t("agents.keepEditing"),
    on_keep: Arc::new(move || keep.set(Pending(None))),
    on_confirm: Arc::new(move || confirm.answer(&asked.request)),
  };
  Some(ctx.mount_keyed::<WidenConfirm>("agent-confirm", question))
}
