//! Where each item of the panel leads, as the Overview's *Needs you* cards lead too
//! (`navigation::waiting::Destination`, `Needs you · where each item leads` `icGHG`), and how it is told from
//! the others while the panel shows.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::{
  attention::{Decided, NeedItem},
  needs_you::{WaitingItem, WaitingKind},
};

use super::super::item::Action;
use crate::{
  navigation::{Navigation, link::Open, press::OpenIn, waiting::Destination},
  ui::{icon::Icon, message::message_text, pressable::Press},
};

/// How an item is told from the others while the panel shows, and its element id's end: its card's key when
/// it is answered in place; else its task's, what it is and its step, as several steps of one run may wait at
/// once; `memory-<project>` for `project`'s memory records.
pub(super) fn key_of(project: ProjectId, item: &WaitingItem) -> Arc<str> {
  if let Some(key) = item.kind.card_key() {
    return key;
  }
  let kind = match item.kind {
    WaitingKind::Memory { .. } => return Arc::from(format!("memory-{project}")),
    WaitingKind::Question(_) => "question",
    WaitingKind::Plan(_) => "plan",
    WaitingKind::Decision(_) => "decision",
    WaitingKind::Interrupted { .. } => "interrupted",
    WaitingKind::StoppedAtLimit { .. } => "limit",
    WaitingKind::ReadyToMerge => "merge",
    WaitingKind::Approval(_) | WaitingKind::Choice(_) => "card",
  };
  let task = item.task.as_ref().map_or_else(String::new, |task| task.id.to_string());
  let step = item.step.as_ref().map_or(0, |step| step.number);
  Arc::from(format!("task-{task}-{kind}-{step}"))
}

/// What the action of `need` of `project`, collapsed and not answered in place, does: opens where it leads and
/// closes the panel, or, where it leads nowhere yet, says why.
pub(super) fn action(
  ctx: &Ctx,
  (project, need): (ProjectId, &NeedItem),
  (navigation, open): (&Navigation, &Signal<bool>),
) -> Action {
  match Destination::of(project, need) {
    Err(reason) => Action::Unavailable(message_text(ctx, &reason)),
    Ok(destination) => {
      let glyph = if need.decided() == Decided::Conversation {
        Icon::MessageCircle
      } else {
        Icon::ArrowUpRight
      };
      Action::Press(glyph, Press::Open(go(navigation, open, destination)))
    }
  }
}

/// Opens `destination` where a press asks (`navigation::link`), pointing at its card, and closes the panel, which
/// the page opening takes the place of; a page opened in a background tab leaves the panel open.
pub(super) fn go(navigation: &Navigation, open: &Signal<bool>, destination: Destination) -> Open {
  let (navigation, open) = (navigation.clone(), open.clone());
  Arc::new(move |at| {
    destination.open(&navigation, at);
    if at != OpenIn::BackgroundTab {
      open.set(false);
    }
  })
}
