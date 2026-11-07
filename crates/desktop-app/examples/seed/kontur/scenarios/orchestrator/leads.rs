//! `orchestrator-leads` (ORC-899): the `orchestrator` scenario with subscriptions that stand, so a verifier reaches
//! what a conversation that leads shows: the header's Leads chips, `/watching` and `/unwatch` with their pickers, and the
//! card of a target another conversation leads. *Lead Webhooks v2, a task and Kontur*, the one the Orchestrator opens
//! on, leads the module Webhooks v2 with an order of its own, the task *Shared rate limiter* (no module) and everything
//! in the project with no order of its own, so the project's default order (Project settings › Orchestrator ›
//! *Watching*) applies to it. *Billing and invoices*, in History, leads the module Billing: a `/watch-module` of Billing
//! from the first conversation shows the already-led card (*Take it here* / *Open that conversation*). Search index is
//! led by none, for a /watch that starts one. Each conversation holds the /watch command and the line the app wrote for
//! it.

use orchester_core::{
  id::ChatId,
  orchestrator::{Chat, Subscription, WatchSettings, WatchTarget, watch::NamedTarget},
};

use super::{
  live, set_up,
  watch::{module, watched},
};
use crate::kontur::{Kontur, SeedError};

/// The project's default order for watching, which the project-wide subscription takes.
pub const DEFAULT_ORDER: &str = "Report failures and questions; ask me before merging anything.";
/// The order of the module Webhooks v2's subscription.
pub const WEBHOOKS_ORDER: &str = "Answer agents from the brief. Ask me before merging into the module branch.";
/// The order of the module Billing's subscription, in the other conversation.
pub const BILLING_ORDER: &str = "Keep the invoices job green.";
/// The conversation that leads a module, a task and the project.
pub const LEADING: &str = "Lead Webhooks v2, a task and Kontur";
/// The conversation that leads Billing.
pub const BILLING: &str = "Billing and invoices";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = set_up(kontur)?;
  let watch = WatchSettings::new(DEFAULT_ORDER, Some(WatchSettings::DEFAULT_NO_PROGRESS_MINUTES))
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.save_orchestrator_watch(&watch)?;
  let (webhooks, _) = module(
    kontur,
    &catalogue,
    "Webhooks v2",
    &["Retry failed deliveries", "Sign every attempt"],
  )?;
  let (billing, _) = module(kontur, &catalogue, "Billing", &["Invoice numbering", "Refund receipts"])?;
  module(kontur, &catalogue, "Search index", &["Index task briefs"])?;
  let alone = live::draft(
    kontur,
    &catalogue,
    ("Shared rate limiter", "One limiter for every outgoing call."),
    kontur.ago(6 * 60),
  )?;
  let task = NamedTarget {
    target: WatchTarget::Task(alone.id()),
    name: Some(alone.key().to_string()),
  };
  let project = NamedTarget {
    target: WatchTarget::Project,
    name: None,
  };

  // Billing's conversation first, so the leading one is the latest and the Orchestrator opens on it.
  let other = Chat::named(ChatId::generate(), BILLING, kontur.ago(60));
  watched(kontur, &other, (vec![billing.clone()], BILLING_ORDER), other.started())?;
  lead(kontur, &other, (&billing, BILLING_ORDER))?;

  let leading = Chat::named(ChatId::generate(), LEADING, kontur.ago(8));
  watched(kontur, &leading, (vec![webhooks.clone()], WEBHOOKS_ORDER), leading.started())?;
  lead(kontur, &leading, (&webhooks, WEBHOOKS_ORDER))?;
  let at = kontur.ago(6);
  watched(kontur, &leading, (vec![task.clone(), project.clone()], ""), at)?;
  lead(kontur, &leading, (&task, ""))?;
  lead(kontur, &leading, (&project, ""))
}

/// The subscription of `chat` to `target` with `order`, since the conversation started.
fn lead(kontur: &mut Kontur, chat: &Chat, (target, order): (&NamedTarget, &str)) -> Result<(), SeedError> {
  let subscription = Subscription::new(chat.id(), target.target, order, chat.started());
  kontur.db.start_subscription(&subscription)?;
  Ok(())
}
