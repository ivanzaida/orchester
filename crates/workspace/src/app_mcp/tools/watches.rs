//! `get_watches`: what the project's Orchestrator conversations lead and what waits there now, as `/watching` shows it
//! ([`WatchReads::watching`]), and who leads a module or a task ([`led_in_json`]) for `get_module` and `get_task`. It
//! only reads: subscribing, stopping and taking a target over stay the person's commands in the conversation.

use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::WatchTarget,
};
use orchester_store::{ChatListing, ProjectDb};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Translate, team, wire};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::Link,
  messages::Message,
  orchestrator::watching::{
    Going, LedIn, TargetView, WaitsFor, WatchOrder, WatchReads, WatchedSubscription, WatchedTask, Watching,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetWatches {
  /// The project's id, or its name.
  pub project: String,
  /// One Orchestrator conversation, by its id or its title; left out, every conversation that leads something.
  #[serde(default)]
  pub conversation: Option<String>,
}

pub async fn get_watches(services: &Services, args: GetWatches, translate: &Translate) -> Result<Value, ToolError> {
  if team::of(services, &args.project).await?.is_some() {
    return Err(team::team_only_on_server("get_watches"));
  }
  let services = services.clone();
  let (project, read) = blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let chats = db.chats()?;
    let reads = WatchReads::read(&db, &open.repo_root)?;
    let shown: Vec<(ChatId, String, Watching)> = match &args.conversation {
      Some(named) => {
        let chat = find_chat(&chats, named)?;
        vec![(
          chat.chat.id(),
          chat.chat.title().to_owned(),
          reads.watching(chat.chat.id()),
        )]
      }
      None => chats
        .iter()
        .filter(|listing| !reads.chat_leads(listing.chat.id()).counts.is_empty())
        .map(|listing| {
          let id = listing.chat.id();
          (id, listing.chat.title().to_owned(), reads.watching(id))
        })
        .collect(),
    };
    Ok((open.project.id(), shown))
  })
  .await?;
  let conversations: Vec<Value> = read
    .iter()
    .map(|(chat, title, watching)| conversation_json(project, (*chat, title), watching, translate))
    .collect();
  Ok(json!({ "conversations": conversations }))
}

/// Who leads a module or a task, as its *Led in* shows it: `null` when no subscription covers it, else the leading
/// conversation, the less specific target it leads through (`via`, `null` when it leads the target itself) and the
/// words the app shows.
pub fn led_in_json(led_in: &LedIn, translate: &Translate) -> Value {
  match led_in {
    LedIn::Nobody => Value::Null,
    LedIn::Chat(leader) => json!({
      "conversation": {
        "id": leader.chat.to_string(),
        "title": &*leader.title,
        "link": &leader.link,
      },
      "via": leader.via.as_ref().map(|via| target_json(via, translate)),
      "shown": translate.text(&leader.words),
    }),
  }
}

/// The conversation of `chats` named by its id or, ignoring case, its title; a title several have is refused.
fn find_chat<'a>(chats: &'a [ChatListing], named: &str) -> Result<&'a ChatListing, ToolError> {
  if let Some(listing) = chats.iter().find(|listing| listing.chat.id().to_string() == named) {
    return Ok(listing);
  }
  let lowered = named.to_lowercase();
  let mut titled = chats
    .iter()
    .filter(|listing| listing.chat.title().to_lowercase() == lowered);
  match (titled.next(), titled.next()) {
    (Some(listing), None) => Ok(listing),
    (Some(_), Some(_)) => Err(ToolError::refused(
      "conversation_ambiguous",
      vec![Message::new("appMcp.errors.conversationAmbiguous").with("conversation", named.to_owned())],
    )),
    (None, _) => Err(ToolError::not_found(Missing::Conversation, named)),
  }
}

fn conversation_json(
  project: ProjectId,
  (chat, title): (ChatId, &str),
  watching: &Watching,
  translate: &Translate,
) -> Value {
  let words = |messages: &[Message]| {
    messages
      .iter()
      .map(|message| translate.text(message))
      .collect::<Vec<_>>()
  };
  let counts = &watching.counts;
  let subscriptions: Vec<Value> = watching
    .subscriptions
    .iter()
    .map(|watched| subscription_json(watched, translate))
    .collect();
  json!({
    "id": chat.to_string(),
    "title": title,
    "link": Link::Orchestrator(project, chat).path(),
    "leads": {
      "modules": counts.modules,
      "tasks": counts.tasks,
      "without_module": counts.without_module,
      "project": counts.project,
      "shown": words(&counts.parts()),
    },
    "total": words(&watching.total),
    "subscriptions": subscriptions,
  })
}

fn subscription_json(watched: &WatchedSubscription, translate: &Translate) -> Value {
  let tasks: Vec<Value> = watched.tasks.iter().map(|task| task_json(task, translate)).collect();
  json!({
    "target": target_json(&watched.target, translate),
    "since": wire::time(watched.since),
    "order": order_json(&watched.order),
    "tasks": tasks,
    "rest": watched.rest.as_ref().map(|rest| translate.text(rest)),
    "unreadable": watched.unreadable.as_ref().map(|why| translate.text(why)),
  })
}

/// A watched target: its kind, the module's or the task's id, its name and its link (`null` for the tasks without a
/// module, which have no page).
fn target_json(view: &TargetView, translate: &Translate) -> Value {
  let (kind, id) = match view.target {
    WatchTarget::Module(module) => ("module", Some(module.to_string())),
    WatchTarget::Task(task) => ("task", Some(task.to_string())),
    WatchTarget::WithoutModule => ("without_module", None),
    WatchTarget::Project => ("project", None),
  };
  json!({
    "kind": kind,
    "id": id,
    "name": translate.text(&view.name),
    "link": &view.link,
  })
}

fn order_json(order: &WatchOrder) -> Value {
  let (source, text) = match order {
    WatchOrder::Own(text) => ("own", Some(&**text)),
    WatchOrder::ProjectDefault(text) => ("project_default", Some(&**text)),
    WatchOrder::None => ("none", None),
  };
  json!({ "source": source, "text": text })
}

fn task_json(task: &WatchedTask, translate: &Translate) -> Value {
  json!({
    "key": &*task.key,
    "id": task.id.to_string(),
    "title": &*task.title,
    "run": task.run.map(|number| number.get()),
    "going": going_json(task.going),
    "state": translate.text(&task.state),
    "note": task.note.as_ref().map(|note| translate.text(note)),
    "action": task.action.as_ref().map(|action| translate.text(action)),
    "link": &task.link,
  })
}

/// What is going on in a task: `running`, `waits` (with what for), `queued` (with its place in the queue from 1),
/// `stopped_at_limit` (whether the daily budget stopped it) or `ready_to_merge`.
fn going_json(going: Going) -> Value {
  match going {
    Going::Running => json!({ "kind": "running" }),
    Going::Waits(waits) => json!({ "kind": "waits", "waits_for": match waits {
      WaitsFor::Question => "question",
      WaitsFor::Message => "message",
      WaitsFor::Plan => "plan",
      WaitsFor::Approval => "approval",
      WaitsFor::Decision => "decision",
      WaitsFor::Run => "run",
    } }),
    Going::Queued { position } => json!({ "kind": "queued", "position": position }),
    Going::StoppedAtLimit { daily_budget } => json!({ "kind": "stopped_at_limit", "daily_budget": daily_budget }),
    Going::ReadyToMerge => json!({ "kind": "ready_to_merge" }),
  }
}
