//! `messages-applied`: a conversation in which the chat proposed messages that move a run and the person applied
//! them, through the chat's own tool path ([`tools::call`]) and the window's Apply ([`proposal::decide`]), before the
//! window opens: a follow-up of the seed's run ready for review (`ExRt1` `gOzsF`), and, after the person applied the
//! chat's cancel of a run at work, a message that ran that task again, whose card shows the run it started. The
//! project's daily budget is lifted first, as Project settings › Limits does, so the runner takes both. The waits and
//! run ends that follow land in another conversation ([`super::proposals::elsewhere`]).

use std::error::Error;

use lurq::app::i18n::I18n;
use orchester_app::services::Services;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, TurnState},
  run::{Run, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::{self, Translate},
  i18n::NAMESPACE,
  orchestrator::{
    proposal::{self, Decision},
    tools::{self, ChatScope},
  },
};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

/// What asks the chat's tools and applies what they propose, in one conversation.
struct Asking<'a> {
  runtime: &'a Runtime,
  services: &'a Services,
  tools: app_mcp::Services,
  translate: Translate,
  scope: ChatScope,
}

/// The conversation, with the session's `services`, in the words of `i18n`.
pub fn conversation(
  runtime: &Runtime,
  (services, i18n): (&Services, I18n),
  prepared: &super::data::Prepared,
) -> Result<ChatId, Box<dyn Error>> {
  let db = ProjectDb::open(&prepared.repo)?;
  let asking = Asking {
    runtime,
    services,
    tools: app_mcp::Services::new(
      services.workspace.clone(),
      services.runners.clone(),
      services.servers.clone(),
      services.programs.clone(),
      (
        services.changes.clone(),
        services.activity.clone(),
        services.jobs.clone(),
      ),
    ),
    translate: Translate::new(move |message| {
      let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
      i18n.t_ns_args(NAMESPACE, message.key, args).to_string()
    }),
    scope: ChatScope {
      project: prepared.project,
      name: db.project().name().as_str().to_owned(),
      root: prepared.repo.clone(),
      chat: ChatId::generate(),
      on_its_own: false,
    },
  };
  let limits = json!({ "project": prepared.project.to_string(), "daily_budget": null });
  runtime
    .block_on(app_mcp::call(
      &asking.tools,
      &asking.translate,
      "update_project_limits",
      object(limits)?,
    ))
    .ok_or("the app has update_project_limits")?
    .map_err(|error| format!("the daily budget was not lifted: {error:?}"))?;
  let (ready, going) = (
    pick(&db, ready_to_merge)?,
    pick(&db, |_, run| Ok(!run.status().is_terminal()))?,
  );
  let text = |run: &Run| json!({ "project": "Kontur", "run": run.id().to_string(), "text": "Also cover the 429 response: wait the Retry-After seconds before the next attempt." });
  asking.ask_and_apply(
    "Follow the ready run up with the 429 case.",
    ("send_message", text(&ready)),
  )?;
  let cancel = json!({ "project": "Kontur", "run": going.id().to_string() });
  asking.ask_and_apply("Cancel the run at work.", ("cancel_run", cancel))?;
  asking.ask_and_apply("Run that task again with the 429 case.", ("send_message", text(&going)))?;
  super::proposals::elsewhere(&mut ProjectDb::open(&prepared.repo)?)?;
  Ok(asking.scope.chat)
}

impl Asking<'_> {
  /// A turn of the person's `text` in which the chat calls `tool` with `arguments`, and the person applies what it
  /// proposed.
  fn ask_and_apply(&self, text: &str, (tool, arguments): (&str, Value)) -> Result<(), Box<dyn Error>> {
    let mut db = ProjectDb::open(&self.scope.root)?;
    let body = MessageBody::new(text)?;
    let chat = db
      .chat(self.scope.chat)?
      .unwrap_or_else(|| Chat::start(self.scope.chat, &body, Timestamp::now()));
    let message = ChatMessageId::generate();
    db.keep_chat_message(&chat, (message, &body), Timestamp::now())?;
    let runners = &self.services.runners;
    self
      .runtime
      .block_on(tools::call(
        (&self.tools, &self.translate, runners),
        &self.scope,
        tool,
        object(arguments)?,
      ))
      .map_err(|error| format!("the chat's {tool} was refused: {error:?}"))?;
    db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())?;
    let seq = db
      .chat_entries(self.scope.chat)?
      .into_iter()
      .rev()
      .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
      .ok_or("the turn kept the proposal")?
      .seq;
    self
      .runtime
      .block_on(proposal::decide(
        (&self.tools, &self.translate, runners),
        &self.scope,
        seq,
        Decision::Accept { widens: Vec::new() },
      ))
      .map_err(|message| format!("the {tool} proposal was not taken: {}", message.key))?;
    Ok(())
  }
}

/// Whether `run` succeeded and was not merged.
fn ready_to_merge(db: &ProjectDb, run: &Run) -> Result<bool, Box<dyn Error>> {
  Ok(*run.status() == RunStatus::Succeeded && db.run_merge(run.id())?.is_none())
}

/// The latest run of the first of the seed's tasks that `wanted`.
fn pick(
  db: &ProjectDb,
  wanted: impl Fn(&ProjectDb, &Run) -> Result<bool, Box<dyn Error>>,
) -> Result<Run, Box<dyn Error>> {
  for task in db.tasks()? {
    if let Some(Ok(run)) = db.runs_of_task(task.id())?.pop()
      && wanted(db, &run)?
    {
      return Ok(run);
    }
  }
  Err("the tasks scenario has such a run".into())
}

fn object(value: Value) -> Result<serde_json::Map<String, Value>, Box<dyn Error>> {
  match value {
    Value::Object(object) => Ok(object),
    _ => Err("the call's arguments are an object".into()),
  }
}
