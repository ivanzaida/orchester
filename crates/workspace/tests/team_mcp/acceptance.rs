//! A team module's acceptance before its delivery, as a local module's (`docs/plan/orchestrator-autonomy/OA-32-…`):
//! its acceptance pipeline and *Deliver when accepted* are chosen on this computer, by a member too, and sent nothing;
//! `start_acceptance` refuses until every task the server holds in the module is done, then makes the acceptance task
//! through `task.create` (an Admin's on the server) in the module, on the team pipeline chosen, keeps it as the
//! module's acceptance and asks for its start; one under way is not made twice.

use orchester_core::connection::AgentProvider;
use serde_json::{Value, json};

use crate::{Checkout, team_server::MODULE_BRANCH, tools::server};

fn module() -> Value {
  json!({"project": "Kontur", "module": "Webhooks"})
}

#[test]
fn team_mcp_acceptance_a_team_modules_choice_is_kept_here_and_sends_nothing() {
  let (server, seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });

  let read = checkout.call("get_module", &module()).expect("read");
  assert_eq!(read["own_branch"]["branch"], MODULE_BRANCH, "{read}");
  assert_eq!(read["own_branch"]["acceptance_pipeline"], Value::Null);
  assert_eq!(read["own_branch"]["waits_for"], "work", "nothing was merged here");
  assert_eq!(
    checkout.refused("start_acceptance", &module()),
    "no_acceptance_pipeline"
  );

  // A member chooses it: kept on this computer, no edit of the module sent.
  let choose = json!({"project": "Kontur", "module": "Webhooks", "acceptance_pipeline": "delivery",
    "deliver_when_accepted": true});
  checkout.call("update_module", &choose).expect("kept here");
  assert_eq!(seen.lock().task_edits, 0, "nothing was sent");
  let read = checkout.call("get_module", &module()).expect("read");
  assert_eq!(read["own_branch"]["acceptance_pipeline"], config.pipeline.to_string());
  assert_eq!(read["own_branch"]["deliver_when_accepted"], true);
  assert_eq!(read["own_branch"]["acceptance"], "none");

  // Only a team pipeline runs a team task's acceptance; taking the pipeline away takes Deliver when accepted with it.
  let own = json!({"project": "Kontur", "module": "Webhooks", "acceptance_pipeline": "nothing-like-it"});
  assert_eq!(checkout.refused("update_module", &own), "team_pipeline_required");
  let none = json!({"project": "Kontur", "module": "Webhooks", "acceptance_pipeline": null});
  checkout.call("update_module", &none).expect("kept here");
  let read = checkout.call("get_module", &module()).expect("read");
  assert_eq!(read["own_branch"]["deliver_when_accepted"], false, "{read}");
}

#[test]
fn team_mcp_acceptance_is_made_on_the_team_server_in_the_module_once_its_tasks_are_done() {
  let (server, seen) = server();
  let config = server.state().config.clone();
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    crate::ready::team_ready(db, &config, AgentProvider::CodexCli);
  });
  let choose = json!({"project": "Kontur", "module": "Webhooks", "acceptance_pipeline": "delivery"});
  checkout.call("update_module", &choose).expect("kept here");

  // KON-7 is open on the server.
  assert_eq!(checkout.refused("start_acceptance", &module()), "tasks_open");
  assert_eq!(seen.lock().task_creates, 0, "nothing was sent");

  // Done there, the acceptance task goes to task.create, which is the Admins': nothing is kept for a member's.
  seen.lock().task_fields = json!({"workflow": "done"});
  assert_eq!(checkout.refused("start_acceptance", &module()), "not_admin");
  assert_eq!(seen.lock().created_pipeline, json!(config.pipeline.to_string()));
  let kept = checkout.db().module_acceptances(server.module).expect("read");
  assert!(kept.is_empty(), "no acceptance kept: {kept:?}");

  // Taken as an Admin's, it is made, moved into the module and kept as the module's acceptance.
  seen.lock().admin_writes = true;
  let started = checkout.call("start_acceptance", &module()).expect("made");
  let made = seen.lock().made[0].clone();
  assert_eq!(started["acceptance"]["task"], made["key"], "{started}");
  assert!(started["acceptance"].get("not_saved").is_none(), "{started}");
  assert_eq!(made["module"], json!(server.module), "made in the module");
  assert_eq!(made["pipeline"], json!(config.pipeline.to_string()));
  assert_eq!(made["title"], "Accept the module Webhooks");
  let brief = made["brief"].as_str().expect("a brief");
  assert!(brief.contains("KON-7") && brief.contains(MODULE_BRANCH), "{brief}");
  let kept = checkout.db().module_acceptances(server.module).expect("read");
  assert_eq!(kept.len(), 1);
  assert_eq!(json!(kept[0].0), made["id"]);

  // It is under way: no second one is made.
  assert_eq!(checkout.refused("start_acceptance", &module()), "acceptance_under_way");
  assert_eq!(seen.lock().made.len(), 1);
  let read = checkout.call("get_module", &module()).expect("read");
  assert_eq!(read["own_branch"]["acceptances"], 1);
}
