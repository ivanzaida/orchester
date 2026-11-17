//! `watch` and `unwatch`, tools of the Orchestrator chat's own (ORC-893): the person asks in words ("watch ORC-889 and
//! ORC-890", "follow the Docs module", "stop watching Docs") and the agent subscribes or unsubscribes its own
//! conversation through the same watch and stop the commands apply ([`super::watch_asked_in`], [`super::stop_in`]),
//! by the same rules: the most specific subscription wins, and a target another conversation leads is not taken over:
//! it is answered as led there, and its card in this conversation leaves *Take it here* to the person. Stopping
//! cancels and pauses nothing. The conversation keeps only the change's line, *at your request*. Refused in a turn
//! Orchester started by itself (`event_turn`), so the agent never subscribes on its own initiative.

use orchester_core::orchestrator::{Chat, WatchTarget, watch::NamedTarget};
use orchester_store::ProjectDb;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::{StopBy, StopWhat, WatchError, stop_in, watch_asked_in};
use crate::{
  app_mcp::{Missing, Services, ToolError, blocking, find_module, find_task},
  messages::Message,
  orchestrator::{led_start::Named, own_project::in_own_project, team_task, tools::ChatScope},
  runner::Runners,
};

/// The tools' names.
pub const WATCH: &str = "watch";
pub const UNWATCH: &str = "unwatch";

/// How a target names the tasks without a module, and every task of the project.
const WITHOUT_MODULE: &str = "without_module";
const PROJECT: &str = "project";

const WATCH_DESCRIPTION: &str = "Subscribes this conversation to targets of this project when the person asks you to \
  in words in this conversation (\"watch ORC-12 and ORC-13\", \"follow the Docs module\"); never on your own \
  initiative. targets: a task's key, a module's name or id, without_module for the tasks without a module, project for \
  every task of the project. order: what the person wants done with their events, in their words; left out, the \
  project's default order. It applies what /watch-task and /watch-module apply, by the same rules: the most specific \
  subscription leads a task's events, and the conversation shows the subscription line at the person's request. \
  Answers started (led here from now on), already_here (led here already; a new order replaces its order), \
  led_elsewhere (another conversation leads it, named by its title: it is not taken, and the card in this \
  conversation lets the person open that conversation or take it here; tell them so) and gone (closed or deleted). \
  Refused in a turn Orchester started by itself (event_turn), and for a name nothing in the project answers to \
  (target_not_found).";

const UNWATCH_DESCRIPTION: &str = "Stops this conversation's subscriptions when the person asks you to in words in \
  this conversation (\"stop watching Docs\"); never on your own initiative. targets: as for watch; all: true stops \
  everything the conversation watches. It applies what /unwatch applies: nothing is canceled or paused, the runs go \
  on, and left_open names the tasks whose work is still open. The conversation shows the stop line at the person's \
  request. Refused in a turn Orchester started by itself (event_turn), for what this conversation does not watch \
  (not_watched) and for what another conversation leads (led_elsewhere).";

/// The tools as the chat's `tools/list` answers.
pub fn tools() -> [Tool; 2] {
  let hints = ToolAnnotations::new()
    .read_only(false)
    .destructive(false)
    .open_world(false);
  [
    Tool::new(WATCH, WATCH_DESCRIPTION, schema_for_type::<WatchTargets>()).with_annotations(hints.clone()),
    Tool::new(UNWATCH, UNWATCH_DESCRIPTION, schema_for_type::<UnwatchTargets>()).with_annotations(hints),
  ]
}

/// Whether `name` is one of these tools.
pub fn is_tool(name: &str) -> bool {
  name == WATCH || name == UNWATCH
}

/// What `watch` takes besides `project`, which may name the chat's own project only.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct WatchTargets {
  /// What to watch: tasks by key (KON-12), modules by name or id, `without_module`, `project`.
  pub targets: Vec<String>,
  /// What the person wants done with their events, in their words; left out, the project's default order.
  #[serde(default)]
  pub order: String,
}

/// What `unwatch` takes besides `project`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UnwatchTargets {
  /// What to stop watching, named as for `watch`.
  #[serde(default)]
  pub targets: Vec<String>,
  /// Everything this conversation watches.
  #[serde(default)]
  pub all: bool,
}

/// Calls `name`, `watch` or `unwatch`, with `arguments` for `scope`'s conversation; `runners` tell the chat it changed.
pub(in crate::orchestrator) async fn call(
  (services, runners): (&Services, &Runners),
  scope: &ChatScope,
  name: &str,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  if scope.on_its_own {
    return Err(ToolError::refused(
      "event_turn",
      vec![Message::new("orchestrator.watch.eventTurn")],
    ));
  }
  in_own_project(scope, &mut arguments)?;
  arguments.remove("project");
  let told = if name == WATCH {
    let asked: WatchTargets = parsed(arguments)?;
    let on_server = on_server(services, scope, &asked.targets).await?;
    let scope = scope.clone();
    blocking(move || watch(&scope, &asked, &on_server)).await?
  } else {
    let asked: UnwatchTargets = parsed(arguments)?;
    let scope = scope.clone();
    blocking(move || unwatch(&scope, &asked)).await?
  };
  runners.chat_changed(&scope.root);
  Ok(told)
}

fn parsed<T: DeserializeOwned>(arguments: JsonObject) -> Result<T, ToolError> {
  serde_json::from_value(Value::Object(arguments)).map_err(|error| ToolError::InvalidArguments(error.to_string()))
}

/// The targets among `named` that name a team task nothing on this computer defines yet (no record of a run, no
/// reading of a watched module), as the team server holds them: each by the name it was asked by. A name the server
/// has no task for is left to [`target`], which says what answers to it.
async fn on_server(
  services: &Services,
  scope: &ChatScope,
  named: &[String],
) -> Result<Vec<(String, NamedTarget)>, ToolError> {
  let (root, asked) = (scope.root.clone(), named.to_vec());
  let unknown = blocking(move || {
    let db = ProjectDb::open(&root)?;
    Ok(
      asked
        .into_iter()
        .map(|named| named.trim().to_owned())
        .filter(|named| !defined_here(&db, named))
        .collect::<Vec<_>>(),
    )
  })
  .await?;
  let mut found = Vec::new();
  for named in unknown {
    match team_task::asked(services, scope, &named).await {
      Ok(Some(task)) => {
        let target = NamedTarget {
          target: WatchTarget::Task(task.id),
          name: Some(task.key),
        };
        found.push((named, target));
      }
      Ok(None) | Err(ToolError::NotFound { .. }) => {}
      Err(refused) => return Err(refused),
    }
  }
  Ok(found)
}

/// Whether `named` is a target this computer can name by itself: one of the two words, a task its records or the
/// server's last reading define, or, when no stored task answers to it, a module.
fn defined_here(db: &ProjectDb, named: &str) -> bool {
  named.eq_ignore_ascii_case(WITHOUT_MODULE)
    || named.eq_ignore_ascii_case(PROJECT)
    || Named::find(db, named).is_ok()
    || (find_task(db, named).is_err() && find_module(db, named).is_ok())
}

fn watch(scope: &ChatScope, asked: &WatchTargets, on_server: &[(String, NamedTarget)]) -> Result<Value, ToolError> {
  let (chat, targets) = resolved(scope, &asked.targets, on_server)?;
  let watched = watch_asked_in(&scope.root, &chat, &targets, &asked.order).map_err(refusal)?;
  let led: Vec<Value> = watched
    .led_elsewhere
    .iter()
    .map(|led| json!({ "target": name(&led.target), "led_in": led.title, "conversation": led.chat.to_string() }))
    .collect();
  Ok(json!({
    "watch": {
      "started": names(&watched.started),
      "already_here": names(&watched.already_here),
      "led_elsewhere": led,
      "gone": names(&watched.gone),
      "order": asked.order.trim(),
    }
  }))
}

fn unwatch(scope: &ChatScope, asked: &UnwatchTargets) -> Result<Value, ToolError> {
  let (chat, targets) = resolved(scope, &asked.targets, &[])?;
  let what = if asked.all {
    StopWhat::All
  } else if targets.is_empty() {
    return Err(refusal(WatchError::NoTarget));
  } else {
    StopWhat::Targets(targets)
  };
  let unwatched = stop_in(&scope.root, &chat, &what, StopBy::Request).map_err(refusal)?;
  Ok(json!({
    "unwatch": { "stopped": names(&unwatched.stopped), "left_open": unwatched.left_open }
  }))
}

/// `scope`'s conversation as kept, and the targets `named` names in its project; a name of `on_server` is the team
/// task the server answered for it.
fn resolved(
  scope: &ChatScope,
  named: &[String],
  on_server: &[(String, NamedTarget)],
) -> Result<(Chat, Vec<NamedTarget>), ToolError> {
  let db = ProjectDb::open(&scope.root)?;
  let chat = db
    .chat(scope.chat)?
    .ok_or_else(|| ToolError::not_found(Missing::Conversation, scope.chat.to_string()))?;
  let targets = named
    .iter()
    .map(|named| {
      let named = named.trim();
      match on_server.iter().find(|(asked, _)| asked == named) {
        Some((_, target)) => Ok(target.clone()),
        None => target(&db, named),
      }
    })
    .collect::<Result<_, _>>()?;
  Ok((chat, targets))
}

/// The target `named` names in `db`: the tasks without a module, the whole project, a task by its key or id (as the
/// project's records or the team server's last reading define it, else as a stored row names it), else a
/// module by its name or id.
fn target(db: &ProjectDb, named: &str) -> Result<NamedTarget, ToolError> {
  if named.eq_ignore_ascii_case(WITHOUT_MODULE) {
    return Ok(NamedTarget {
      target: WatchTarget::WithoutModule,
      name: None,
    });
  }
  if named.eq_ignore_ascii_case(PROJECT) {
    return Ok(NamedTarget {
      target: WatchTarget::Project,
      name: None,
    });
  }
  // A team task that never ran here has no stored row to be found by: the server's last reading names it once a
  // subscription watches it or its module.
  if let Ok(task) = Named::find(db, named) {
    return Ok(NamedTarget {
      target: WatchTarget::Task(task.id),
      name: Some(task.key),
    });
  }
  if let Ok(task) = find_task(db, named) {
    return Ok(NamedTarget {
      target: WatchTarget::Task(task.id()),
      name: Some(task.key().to_string()),
    });
  }
  match find_module(db, named) {
    Ok(module) => Ok(NamedTarget {
      target: WatchTarget::Module(module.id()),
      name: Some(module.name().as_str().to_owned()),
    }),
    Err(_) => Err(ToolError::refused(
      "target_not_found",
      vec![Message::new("orchestrator.watch.unknownTarget").with("target", named.to_owned())],
    )),
  }
}

/// Why the watch or the stop was refused, with a stable code.
fn refusal(error: WatchError) -> ToolError {
  let code = match error {
    WatchError::Store(store) => return store.into(),
    WatchError::NoTarget => "no_target",
    WatchError::Gone(_) => "gone",
    WatchError::NotWatched => "not_watched",
    WatchError::LedElsewhere { .. } => "led_elsewhere",
    WatchError::AlreadyHere => "already_here",
    // No tool of the agent's changes a ceiling, so none of them is refused this way; the code names it all the same.
    WatchError::NotLedToEnd => "not_led_to_end",
  };
  ToolError::refused(code, vec![error.message()])
}

/// `named` as the agent reads it: a task's key, a module's name, or the words the tool takes.
fn name(named: &NamedTarget) -> String {
  match (&named.name, named.target) {
    (Some(name), _) => name.clone(),
    (None, WatchTarget::WithoutModule) => WITHOUT_MODULE.to_owned(),
    (None, WatchTarget::Project) => PROJECT.to_owned(),
    (None, WatchTarget::Task(task)) => task.to_string(),
    (None, WatchTarget::Module(module)) => module.to_string(),
  }
}

fn names(named: &[NamedTarget]) -> Vec<String> {
  named.iter().map(name).collect()
}
