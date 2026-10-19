//! `continue` and `continue-applied`: a conversation where the Orchestrator's agent asked to continue the seed's run
//! that waits for a decision after Review used up its retries with one more retry (`continue_run`, through the chat's
//! own tool path), before the window opens. In `continue` the proposal waits for the person; a verifier's press on
//! *Allow retry* is refused as synthetic input. In `continue-applied` the person's *Allow retry* is taken as the
//! window takes it ([`proposal::decide`]), so the runner retries the step, and the run's end that follows lands in
//! the conversation. A verifier driving the window cannot apply a proposal itself: only a person does.

use std::error::Error;

use lurq::app::i18n::I18n;
use orchester_app::services::Services;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, TurnState},
  store::{RunStore, TaskStore},
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
use serde_json::json;
use tokio::runtime::Runtime;

/// The seed's task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";

/// The conversation in which the chat proposed one more retry of the stuck run, with the session's `services`,
/// explaining a refusal in the words of `i18n`; applied as the person's *Allow retry* when `apply`.
pub fn conversation(
  runtime: &Runtime,
  (services, i18n): (&Services, I18n),
  prepared: &super::data::Prepared,
  apply: bool,
) -> Result<ChatId, Box<dyn Error>> {
  let mut db = ProjectDb::open(&prepared.repo)?;
  let number = stuck_run(&db)?;
  let text = MessageBody::new(&format!("Give run #{number} of the cache task one more try."))?;
  let chat = Chat::start(ChatId::generate(), &text, Timestamp::now());
  let message = ChatMessageId::generate();
  db.keep_chat_message(&chat, (message, &text), Timestamp::now())?;
  let scope = ChatScope {
    project: prepared.project,
    name: db.project().name().as_str().to_owned(),
    root: prepared.repo.clone(),
    chat: chat.id(),
  };
  let tools = app_mcp::Services::new(
    services.workspace.clone(),
    services.runners.clone(),
    services.servers.clone(),
    services.programs.clone(),
    (
      services.changes.clone(),
      services.activity.clone(),
      services.jobs.clone(),
    ),
  );
  let translate = Translate::new(move |message| {
    let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
    i18n.t_ns_args(NAMESPACE, message.key, args).to_string()
  });
  let arguments = json!({ "project": "Kontur", "run": number.to_string(), "choice": { "kind": "allow_retry" } });
  let serde_json::Value::Object(arguments) = arguments else {
    return Err("the call's arguments are an object".into());
  };
  runtime
    .block_on(tools::call(
      (&tools, &translate, &services.runners),
      &scope,
      "continue_run",
      arguments,
    ))
    .map_err(|error| format!("the chat's continue_run was refused: {error:?}"))?;
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())?;
  if apply {
    let seq = db
      .chat_entries(chat.id())?
      .into_iter()
      .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
      .ok_or("the conversation has the proposal")?
      .seq;
    runtime
      .block_on(proposal::decide(
        (&tools, &translate, &services.runners),
        &scope,
        seq,
        Decision::Accept { widens: Vec::new() },
      ))
      .map_err(|message| format!("the retry was not taken: {}", message.key))?;
  }
  Ok(chat.id())
}

/// The number of the seed's run that waits for a decision.
fn stuck_run(db: &ProjectDb) -> Result<u32, Box<dyn Error>> {
  let task = db
    .tasks()?
    .into_iter()
    .find(|task| task.details().title.as_str() == STUCK)
    .ok_or("the tasks scenario has the stuck task")?;
  let run = db
    .runs_of_task(task.id())?
    .pop()
    .ok_or("the stuck task has a run")?
    .map_err(|error| format!("the stuck task's run cannot be read: {error:?}"))?;
  Ok(run.number().get())
}
