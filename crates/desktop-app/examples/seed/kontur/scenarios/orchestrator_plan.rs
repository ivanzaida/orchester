//! The `orchestrator` scenario in plan-only mode (ORC-646, `12`–`12d` in `Orchestrator · redesign` `hujxu`), on the
//! conversation the Orchestrator opens on:
//! - `orchestrator-plan-only`: its *Plan only* switch on, nothing asked yet (`12` `SDd2A`);
//! - `orchestrator-plan-answer`: a message sent in plan-only mode, *Delivered · plan only*, answered with a plan of
//!   four steps that waits for the person (`12b` `X7BYz`);
//! - `orchestrator-plan-approved`: that plan approved a minute later and being carried out: three steps done, the turn
//!   still working (`12c` `j7Oa9v`). A runner ends a turn it does not carry out as interrupted once it looks after the
//!   project, so a QA copy shows it working only without one;
//! - `orchestrator-plan-changed`: the plan replaced by a second one after the person changed it, and a third dismissed
//!   (`12d` `y79oJ7`'s card states).

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{ActionOutcome, Chat, ChatAction, ChatPlan, PlanStep, StepKind, TurnMode, TurnState},
  resource::ModelId,
  run::Answerer,
  time::Timestamp,
};

use super::orchestrator;
use crate::kontur::{Kontur, SeedError};

/// What the person asked for in plan-only mode.
const ASKED: &str = "Continue the interrupted runs, draft a task for per-endpoint retry settings, and let three runs go at \
                     once.";
/// What the Orchestrator said over its plan.
const SAID: &str = "Here is what I would do. Nothing has started: approve it, or tell me what to change.";
/// The model the Orchestrator runs with.
const MODEL: &str = "claude-opus-5-5";

pub fn plan_only(kontur: &mut Kontur) -> Result<(), SeedError> {
  switched_on(kontur).map(drop)
}

pub fn answer(kontur: &mut Kontur) -> Result<(), SeedError> {
  let chat = switched_on(kontur)?;
  planned(kontur, chat, (ASKED, 2), &four_steps()?).map(drop)
}

pub fn approved(kontur: &mut Kontur) -> Result<(), SeedError> {
  let chat = switched_on(kontur)?;
  let seq = planned(kontur, chat, (ASKED, 2), &four_steps()?)?;
  let at = kontur.ago(1);
  let carrying = ChatMessageId::generate();
  kontur
    .db
    .approve_chat_plan(seq, (carrying, Answerer::Person), at)?
    .ok_or_else(|| SeedError::Invalid("the plan waits".to_owned()))?;
  for (tool, task) in [
    ("resume_run", "KON-12"),
    ("resume_run", "KON-21"),
    ("create_task", "KON-31"),
  ] {
    let action = ChatAction::new(tool, Some(task.to_owned()), ActionOutcome::Done);
    kontur.db.add_chat_action(chat, &action, at)?;
  }
  Ok(())
}

pub fn changed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let chat = switched_on(kontur)?;
  planned(kontur, chat, (ASKED, 6), &four_steps()?)?;
  let change = "Leave run #12 for now, and keep two runs at once.";
  planned(kontur, chat, (change, 4), &three_steps()?)?;
  let seq = planned(kontur, chat, ("Draft the follow-up only.", 2), &three_steps()?)?;
  if kontur.db.dismiss_chat_plan(seq, kontur.ago(1))? {
    Ok(())
  } else {
    Err(SeedError::Invalid("the plan waits".to_owned()))
  }
}

/// The `orchestrator` scenario with the *Plan only* switch of the conversation it opens on turned on; that
/// conversation.
fn switched_on(kontur: &mut Kontur) -> Result<ChatId, SeedError> {
  orchestrator::seed(kontur)?;
  let chat = kontur
    .db
    .latest_chat()?
    .ok_or_else(|| SeedError::Invalid("the Orchestrator's conversation".to_owned()))?;
  kontur.db.set_chat_plan_only(chat, true)?;
  Ok(chat)
}

/// A turn of `chat` the person sent in plan-only mode with `asked` `minutes` ago, answered with [`SAID`] and `plan`;
/// the plan's entry.
fn planned(
  kontur: &mut Kontur,
  chat: ChatId,
  (asked, minutes): (&str, i64),
  plan: &ChatPlan,
) -> Result<i64, SeedError> {
  let kept = kontur
    .db
    .chat(chat)?
    .ok_or_else(|| SeedError::Invalid("the Orchestrator's conversation".to_owned()))?;
  let at = kontur.ago(minutes);
  let message = ChatMessageId::generate();
  keep(kontur, &kept, (message, asked), at)?;
  let model = ModelId::new(MODEL).map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.add_chat_reply(chat, (SAID, &model), at)?;
  let seq = kontur.db.add_chat_plan(chat, plan, at)?;
  kontur.db.end_chat_turn(message, &TurnState::Answered, at)?;
  Ok(seq)
}

/// Keeps the person's message `message` saying `asked` in `chat` at `at`, its turn planning.
fn keep(
  kontur: &mut Kontur,
  chat: &Chat,
  (message, asked): (ChatMessageId, &str),
  at: Timestamp,
) -> Result<(), SeedError> {
  let body = MessageBody::new(asked).map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur.db.keep_chat_message_in(
    chat,
    (message, &body),
    (&[], Answerer::Person),
    (&TurnState::Thinking, at),
    TurnMode::Plans,
  )?;
  Ok(())
}

/// The plan of `12b`: two runs continued, a draft and a setting.
fn four_steps() -> Result<ChatPlan, SeedError> {
  plan(
    &[
      (StepKind::Run, "Continue run #8", "KON-12 · from its last checkpoint"),
      (StepKind::Run, "Continue run #12", "KON-21 · from its last checkpoint"),
      (
        StepKind::Draft,
        "Create a draft",
        "Per-endpoint retry settings · Feature delivery v3",
      ),
      (
        StepKind::Setting,
        "Change a project setting",
        "Runs · parallel runs 2 → 3",
      ),
    ],
    "Run #11 of KON-16: its branch conflicts with main, so continuing it needs you first",
  )
}

/// The changed plan: one run and the draft.
fn three_steps() -> Result<ChatPlan, SeedError> {
  plan(
    &[
      (StepKind::Run, "Continue run #8", "KON-12 · from its last checkpoint"),
      (
        StepKind::Draft,
        "Create a draft",
        "Per-endpoint retry settings · Feature delivery v3",
      ),
      (StepKind::Setting, "Keep parallel runs", "Runs · parallel runs 2"),
    ],
    "Run #12 of KON-21, as you asked; run #11 of KON-16 still needs you first",
  )
}

fn plan(steps: &[(StepKind, &str, &str)], left_out: &str) -> Result<ChatPlan, SeedError> {
  let steps = steps
    .iter()
    .filter_map(|(kind, action, target)| PlanStep::new(*kind, action, Some(target)))
    .collect();
  ChatPlan::new(
    steps,
    Some(left_out),
    Some("2 runs and 1 draft on Claude Code · Work · about 400k tokens"),
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))
}
