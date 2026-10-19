//! `proposal-refused`: the person's *Start* on the proposal of a task edited since it was proposed, taken as the
//! window takes it ([`proposal::decide`]) before the window opens, so its card shows the refusal. A verifier driving
//! the window cannot press *Start* itself: only a person starts a proposed run, and synthetic input is refused.

use std::error::Error;

use lurq::app::i18n::I18n;
use orchester_app::services::Services;
use orchester_core::{
  id::ChatId,
  orchestrator::{ActionOutcome, ChatAction, EntryKind},
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::{self, Translate},
  i18n::NAMESPACE,
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
};
use tokio::runtime::Runtime;

/// Takes *Start* on the start proposed in `chat` of the project at `prepared`, with the session's `services`,
/// explaining a refusal in the words of `i18n`; an error unless the start was refused as changed since proposed.
pub fn start(
  runtime: &Runtime,
  (services, i18n): (&Services, I18n),
  prepared: &super::data::Prepared,
  chat: ChatId,
) -> Result<(), Box<dyn Error>> {
  let db = ProjectDb::open(&prepared.repo)?;
  let seq = db
    .chat_entries(chat)?
    .into_iter()
    .find(|entry| {
      matches!(
        &entry.kind,
        EntryKind::Action(ChatAction {
          outcome: ActionOutcome::Proposed(_),
          ..
        })
      )
    })
    .ok_or("the conversation has a proposed start")?
    .seq;
  let scope = ChatScope {
    project: prepared.project,
    name: db.project().name().as_str().to_owned(),
    root: prepared.repo.clone(),
    chat,
    on_its_own: false,
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
  runtime
    .block_on(proposal::decide(
      (&tools, &translate, &services.runners),
      &scope,
      seq,
      Decision::Accept { widens: Vec::new() },
    ))
    .map_err(|message| format!("the start was not taken: {}", message.key))?;
  let refused = db.chat_entries(chat)?.into_iter().any(|entry| {
    matches!(&entry.kind, EntryKind::Action(ChatAction {
      outcome: ActionOutcome::Refused { code, .. },
      ..
    }) if code == "changed_since_proposed")
  });
  if refused {
    Ok(())
  } else {
    Err("the start was not refused as changed since proposed".into())
  }
}
