//! `get_decisions`: what the project's Orchestrator decided on its own, with its reasons
//! (`orchester_core::orchestrator::decision`, `docs/plan/orchestrator-autonomy/OA-22-…`). Reading only: marking a
//! decision seen and overruling it are the person's, in the app.

use std::collections::HashSet;

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::ProjectId,
  orchestrator::decision::{Decision, Review, Undo},
};
use orchester_store::{ChatListing, DecisionsRead, ProjectDb};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{daily_report::local_day_start, find_module, watches::find_chat, wire};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::Link,
  tasks::recorded,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetDecisions {
  /// The project's id, or its name.
  pub project: String,
  /// Only the decisions on this task, by its key.
  #[serde(default)]
  pub task: Option<String>,
  /// Only the decisions on the tasks of this module, by its id or its name.
  #[serde(default)]
  pub module: Option<String>,
  /// Only the decisions of this Orchestrator conversation, by its id or its title.
  #[serde(default)]
  pub conversation: Option<String>,
  /// Only the decisions of this local day, `2026-10-20`.
  #[serde(default)]
  pub day: Option<String>,
  /// Only those the person has not looked at.
  #[serde(default)]
  pub open_only: bool,
}

pub async fn get_decisions(services: &Services, args: GetDecisions) -> Result<Value, ToolError> {
  let day = args.day.as_deref().map(day_of).transpose()?;
  let services = services.clone();
  blocking(move || {
    let (open, db) = services.read_project(&args.project)?;
    let chats = db.chats()?;
    let chat = match &args.conversation {
      Some(named) => Some(find_chat(&chats, named)?.chat.id()),
      None => None,
    };
    let read = DecisionsRead {
      open_only: args.open_only,
      task: args.task.clone(),
      chat,
      since: day.map(|(since, _)| since),
      until: day.map(|(_, until)| until),
    };
    let mut decisions = db.decisions(&read)?;
    if let Some(named) = &args.module {
      let keys = module_keys(&db, named)?;
      decisions.retain(|decision| {
        let task = decision.made.task.as_deref().map(str::to_lowercase);
        task.is_some_and(|task| keys.contains(&task))
      });
    }
    let open_count = decisions.iter().filter(|decision| decision.review.is_open()).count();
    let listed: Vec<Value> = decisions
      .iter()
      .map(|decision| decision_json(open.project.id(), &chats, decision))
      .collect();
    Ok(json!({ "decisions": listed, "open": open_count }))
  })
  .await
}

/// When local day `text` starts and ends.
fn day_of(text: &str) -> Result<(orchester_core::time::Timestamp, orchester_core::time::Timestamp), ToolError> {
  let wrong = || ToolError::InvalidArguments(format!("`day` is a local day such as 2026-10-20, not `{text}`"));
  let day = NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| wrong())?;
  let next = day.checked_add_days(Days::new(1)).ok_or_else(wrong)?;
  match (local_day_start(day), local_day_start(next)) {
    (Some(since), Some(until)) => Ok((since, until)),
    _ => Err(wrong()),
  }
}

/// The keys, in lower case, of the tasks in the module `named` names.
fn module_keys(db: &ProjectDb, named: &str) -> Result<HashSet<String>, ToolError> {
  let module = find_module(db, named)?.id();
  Ok(
    recorded::all(db)?
      .into_iter()
      .filter(|task| task.module() == Some(module))
      .map(|task| task.key().to_string().to_lowercase())
      .collect(),
  )
}

fn decision_json(project: ProjectId, chats: &[ChatListing], decision: &Decision) -> Value {
  let made = &decision.made;
  // A conversation deleted since keeps its decisions; its title is then not known.
  let title = chats
    .iter()
    .find(|listing| listing.chat.id() == made.chat)
    .map(|listing| listing.chat.title().to_owned());
  let reviewed_at = match decision.review {
    Review::Open => None,
    Review::Seen { at } | Review::Overruled { at } => Some(wire::time(at)),
  };
  json!({
    "number": decision.number,
    "tool": made.tool,
    "task": made.task,
    "run": made.run.map(|run| run.to_string()),
    "reason": made.reason.as_str(),
    "conversation": {
      "id": made.chat.to_string(),
      "title": title,
      "link": Link::Orchestrator(project, made.chat).path(),
    },
    "at": wire::time(made.at),
    "undo": made.undo.map(Undo::name),
    "review": decision.review.name(),
    "reviewed_at": reviewed_at,
  })
}
