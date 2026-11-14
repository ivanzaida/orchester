//! `get_standing`: where what the project's Orchestrator conversations watch stands
//! (`crate::orchestrator::standing`, `docs/plan/orchestrator-autonomy/OA-12-…`): each subscription's tasks, each as
//! done, working, waiting and for what, ready to start, blocked, failed or parked, and the target as a whole; and, for
//! one that leads to the end, what would stop or weaken it while nobody is at the computer
//! (`crate::orchestrator::leaving`, `docs/plan/orchestrator-autonomy/OA-19-…`). Reading only, and the same for a
//! local and a team project.

use orchester_core::{
  id::ProjectId,
  orchestrator::{
    WatchTarget,
    parking::Parking,
    standing::{TargetStanding, TaskStanding},
  },
};
use orchester_store::ChatListing;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Translate, watches::find_chat, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  orchestrator::{
    leaving::{AppSet, Leaving, leaving_in},
    standing::{Led, Reads},
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetStanding {
  /// The project's id, or its name.
  pub project: String,
  /// One Orchestrator conversation, by its id or its title; left out, every conversation that watches something.
  #[serde(default)]
  pub conversation: Option<String>,
}

pub async fn get_standing(services: &Services, args: GetStanding, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, chats, read) = blocking(move || {
    let (open, db) = services.read_project(&args.project)?;
    let chats = db.chats()?;
    let only = match &args.conversation {
      Some(named) => Some(find_chat(&chats, named)?.chat.id()),
      None => None,
    };
    let app = AppSet {
      drives: services.workspace.orchestrator_drives(&open.repo_root)?,
      keeps_awake: services.workspace.keeps_awake()?,
    };
    let mut read = Vec::new();
    for led in Reads::read(&db)?.all(&db)? {
      if only.is_some_and(|chat| led.subscription.chat() != chat) {
        continue;
      }
      // Only a subscription that leads to the end is left to itself, so only it is read for what would stop it.
      let leaving = match led.subscription.mode().ceilings() {
        Some(_) => Some(
          leaving_in((&open.repo_root, &services.programs), &led.subscription, app)
            .map_err(|message| ToolError::refused("unreadable", vec![message]))?,
        ),
        None => None,
      };
      read.push((led, leaving));
    }
    Ok((open.project.id(), chats, read))
  })
  .await?;
  let subscriptions: Vec<Value> = read
    .iter()
    .map(|(led, leaving)| {
      let mut told = led_json(project, &chats, led);
      told["before_you_leave"] = leaving
        .as_ref()
        .map_or(Value::Null, |leaving| leaving_json(leaving, translate));
      told
    })
    .collect();
  Ok(json!({ "subscriptions": subscriptions }))
}

/// What would stop or weaken a leading subscription while nobody is there: whether it is ready to be left, each line
/// by its name with its words and the tasks it names, and what the app cannot check.
fn leaving_json(leaving: &Leaving, translate: &Translate) -> Value {
  let weak: Vec<Value> = leaving
    .weak
    .iter()
    .map(|weak| {
      json!({
        "kind": weak.name(),
        "tasks": weak.tasks(),
        "shown": translate.text(&weak.message()),
      })
    })
    .collect();
  let not_checked: Vec<String> = leaving
    .not_checked
    .iter()
    .map(|not| translate.text(&not.message()))
    .collect();
  json!({ "ready": leaving.ready(), "weak": weak, "not_checked": not_checked })
}

fn led_json(project: ProjectId, chats: &[ChatListing], led: &Led) -> Value {
  let chat = led.subscription.chat();
  let title = chats
    .iter()
    .find(|listing| listing.chat.id() == chat)
    .map(|listing| listing.chat.title().to_owned());
  let (kind, id) = match led.subscription.target() {
    WatchTarget::Module(module) => ("module", Some(module.to_string())),
    WatchTarget::Task(task) => ("task", Some(task.to_string())),
    WatchTarget::WithoutModule => ("without_module", None),
    WatchTarget::Project => ("project", None),
  };
  let (can_start, slot_free, budget_used) = match &led.standing.target {
    TargetStanding::CanStart { tasks, slot_free } => (tasks.clone(), Some(*slot_free), false),
    TargetStanding::Stuck { budget_used } => (Vec::new(), None, *budget_used),
    TargetStanding::Working | TargetStanding::Done | TargetStanding::ToDeliver(_) => (Vec::new(), None, false),
  };
  let waits_for = match led.standing.target {
    TargetStanding::ToDeliver(step) => Some(step.name()),
    _ => None,
  };
  let tasks: Vec<Value> = led
    .standing
    .tasks
    .iter()
    .map(|(task, standing)| {
      let parking = led
        .parked
        .iter()
        .find(|(key, _)| key == task)
        .map(|(_, parking)| parking);
      task_json(task, standing, parking)
    })
    .collect();
  json!({
    "conversation": { "id": chat.to_string(), "title": title, "link": Link::Orchestrator(project, chat).path() },
    "target": { "kind": kind, "id": id, "name": led.name },
    "leads_to_the_end": led.subscription.mode().ceilings().is_some(),
    "standing": led.standing.target.name(),
    "can_start": can_start,
    "slot_free": slot_free,
    "budget_used": budget_used,
    "waits_for": waits_for,
    "tasks": tasks,
  })
}

fn task_json(task: &str, standing: &TaskStanding, parking: Option<&Parking>) -> Value {
  let (blocked_by, runs, again) = match standing {
    TaskStanding::BlockedBy(by) => (Some(by.clone()), None, None),
    TaskStanding::Failed { runs, again } => (None, Some(*runs), Some(*again)),
    _ => (None, None, None),
  };
  json!({
    "task": task,
    "standing": standing.name(),
    "blocked_by": blocked_by,
    "runs": runs,
    "may_run_again": again,
    "parked": parking.map(|parking| json!({
      "needs": parking.needs.name(),
      "why": parking.why.as_str(),
      "to_do": parking.to_do.as_str(),
      "since": wire::time(parking.at),
      "run": parking.run.map(|run| run.to_string()),
    })),
  })
}
