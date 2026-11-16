//! Changing how a subscription takes its target's work, from the `/watching` answer (`L02` `HpcQJ`, `Parts · Ceilings`
//! on the board `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`): a target led
//! to the end is only followed from now on ([`follow_in`]), or led on within other ceilings ([`set_ceilings_in`]). Both
//! are the person's alone: no tool of the agent's reaches them. The subscription stays the same one, since when it
//! was, so what its work spent goes on counting; nothing that runs is stopped, and nothing new is started by the
//! change. Leading a target the conversation only follows is the /watch command's
//! ([`super::lead_in`]), which starts its budget anew.

use std::path::Path;

use orchester_core::{
  id::ChatId,
  orchestrator::{Ceilings, Subscription, WatchMode, WatchTarget},
};
use orchester_store::ProjectDb;

use super::{WatchError, read};

/// Conversation `chat` only follows `target` from now on, in the project at `root`; one it follows already stays so.
/// Refused when it does not lead `target`. Blocks on SQLite.
pub fn follow_in(root: &Path, chat: ChatId, target: WatchTarget) -> Result<(), WatchError> {
  let mut db = ProjectDb::open(root)?;
  led_here(&db, chat, target)?;
  changed(db.set_subscription_mode(chat, target, WatchMode::Follow)?)
}

/// Conversation `chat` leads `target` to the end within `ceilings` from now on, in the project at `root`. Refused when
/// it does not lead `target`, or only follows it. Blocks on SQLite.
pub fn set_ceilings_in(root: &Path, chat: ChatId, target: WatchTarget, ceilings: Ceilings) -> Result<(), WatchError> {
  let mut db = ProjectDb::open(root)?;
  if led_here(&db, chat, target)?.mode().ceilings().is_none() {
    return Err(WatchError::NotLedToEnd);
  }
  changed(db.set_subscription_mode(chat, target, WatchMode::Lead { ceilings })?)
}

/// `chat`'s subscription to `target`; the refusal when it has none.
fn led_here(db: &ProjectDb, chat: ChatId, target: WatchTarget) -> Result<Subscription, WatchError> {
  match db.active_subscriptions()?.of(target) {
    Some(led) if led.chat() == chat => Ok(led.clone()),
    Some(led) => Err(WatchError::LedElsewhere {
      chat: led.chat(),
      title: read::title(db, led.chat())?,
    }),
    None => Err(WatchError::NotWatched),
  }
}

/// A change the store applied; one it found nothing to apply to (the subscription ended meanwhile) is refused.
fn changed(now: Option<Subscription>) -> Result<(), WatchError> {
  now.map(|_| ()).ok_or(WatchError::NotWatched)
}
