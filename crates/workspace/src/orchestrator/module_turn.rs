//! What a turn Orchester started by itself may do about a module's acceptance and its delivery
//! (`docs/plan/orchestrator-autonomy/OA-20-…`, `OA-32-…`). `start_acceptance` and `deliver_module` are carried out
//! in such a turn only for a module its conversation leads to the end (`not_led`), and only at the step the module
//! waits for (`not_due`): an acceptance once its every task is done and none stands for the branch as it is, within
//! the acceptances a conversation starts by itself; a delivery once that acceptance passed and *Deliver when accepted*
//! is on for the module. A team module's delivery is asked for once its every task is done: its team server decides
//! it, by an Admin's authorization or the team's automatic merge, and the chat authorizes nothing. In a turn a
//! person's message started, both go to the tool as they are: the person asked.

use orchester_core::{
  module::delivery::DeliveryStep,
  orchestrator::{WatchTarget, standing::TargetStanding},
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{standing::Reads, tools::ChatScope};
use crate::{
  app_mcp::{ToolError, blocking, find_module},
  messages::Message,
};

/// The app's tool that delivers a module.
pub(super) const DELIVER_MODULE: &str = "deliver_module";
/// The app's tool that starts a module's acceptance.
pub(super) const START_ACCEPTANCE: &str = "start_acceptance";

/// Refuses the chat's call of `name` ([`DELIVER_MODULE`] or [`START_ACCEPTANCE`]) with `arguments` in `scope`'s
/// turn, which nobody asked for, unless its conversation leads the module to the end and the module waits for that
/// step now. A call that names no module is left to the tool's own refusal.
pub(super) async fn checked(scope: &ChatScope, name: &str, arguments: &JsonObject) -> Result<(), ToolError> {
  let Some(named) = arguments.get("module").and_then(Value::as_str).map(str::to_owned) else {
    return Ok(());
  };
  let (root, chat, delivers) = (scope.root.clone(), scope.chat, name == DELIVER_MODULE);
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let module = find_module(&db, &named)?.id();
    let subscriptions = db.active_subscriptions()?;
    let leading = subscriptions
      .of(WatchTarget::Module(module))
      .filter(|subscription| subscription.chat() == chat && subscription.mode().ceilings().is_some());
    let Some(leading) = leading else {
      return Err(ToolError::refused(
        "not_led",
        vec![Message::new("orchestrator.module.notLed")],
      ));
    };
    let stands = Reads::read(&db)?.led(&db, leading)?.standing.target;
    let wanted = if delivers {
      DeliveryStep::Deliver
    } else {
      DeliveryStep::Accept
    };
    // A team module has no acceptance kept here: its delivery is asked for once its every task is done.
    let due = stands == TargetStanding::ToDeliver(wanted)
      || (delivers && stands == TargetStanding::Done && db.sharing()?.is_some());
    if due {
      return Ok(());
    }
    let waits = match stands {
      TargetStanding::ToDeliver(step) => step.name(),
      other => other.name(),
    };
    Err(ToolError::refused(
      "not_due",
      vec![Message::new("orchestrator.module.notDue").with("standing", waits.to_owned())],
    ))
  })
  .await
}
