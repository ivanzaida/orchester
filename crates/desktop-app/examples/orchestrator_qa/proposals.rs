//! `proposals`: a conversation with the proposals of turns a wait or a run's end started by itself
//! (`Orchestrator · proposals in automatic turns · states` `ExRt1`), seeded as the chat keeps them: a pause waiting,
//! applied, dismissed and refused because its run ended; a resume waiting, applied and refused on a run stopped at
//! its token budget; a release of new starts waiting and applied; and a message refused because its run changed since
//! it was proposed. Each card reads its run's facts from the seed as it stands; a refusal keeps the words the apply
//! path gives it, in the window's language; then messages that would move a run, waiting for the person: a follow-up, a
//! continuation, a resume, one that waits for a raised limit and one that runs the task again. One turn holds every
//! card, in this order (`-0` to `-14` of its action ids),
//! so a verifier scrolls to each; the waits and run ends that follow land in another conversation ([`elsewhere`]).

use std::error::Error;

use lurq::app::i18n::I18n;
use orchester_core::{
  id::ChatId,
  orchestrator::{
    ActionOutcome, ChatAction, TurnState,
    change::{Change, MessageThen, ProposedChange},
  },
  run::{PauseReason, Run, RunLimit, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{i18n::NAMESPACE, messages::Message};

use super::data::{Part, Turn, ago, keep};

/// The conversation, kept in the project at `prepared`, its refusals in the words of `i18n`.
pub fn conversation(prepared: &super::data::Prepared, i18n: &I18n) -> Result<ChatId, Box<dyn Error>> {
  let mut db = ProjectDb::open(&prepared.repo)?;
  let connection = db
    .orchestrator_settings()?
    .ok_or("the Orchestrator runs on the seed's connection")?
    .connection();
  let runs = latest_runs(&db)?;
  let going = pick(&runs, |run| !run.status().is_terminal())?;
  let ended = pick(&runs, |run| run.status().is_terminal())?;
  let limited = pick(&runs, |run| {
    matches!(
      run.status(),
      RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
    )
  })?;
  let words = |key: &'static str, run: &(Run, String)| -> String {
    let message = Message::new(key).with("run", run.0.number().get().to_string());
    let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
    i18n.t_ns_args(NAMESPACE, message.key, args).to_string()
  };
  let refused = |key: &'static str, code: &str, run: &(Run, String)| ActionOutcome::Refused {
    code: code.to_owned(),
    reason: words(key, run),
  };
  let pause = |run: &(Run, String)| Change::Pause {
    run: run.0.id(),
    number: run.0.number().get(),
  };
  let resume = |run: &(Run, String)| Change::Resume {
    run: run.0.id(),
    number: run.0.number().get(),
  };
  let message = |run: &(Run, String)| Change::Message {
    run: run.0.id(),
    number: run.0.number().get(),
    text: TEXT.to_owned(),
    now: false,
    then: None,
    sent_as: Some("send_after_turn".to_owned()),
  };
  // In the order the board draws them; one turn holds them all, so each is on screen with the conversation's row.
  let mut cards = vec![
    action(("pause_run", going), pause(going), ActionOutcome::Waits),
    settled(action(("pause_run", going), pause(going), ActionOutcome::Applied)),
    settled(action(("pause_run", going), pause(going), ActionOutcome::Dismissed)),
    settled(action(
      ("pause_run", ended),
      pause(ended),
      refused("orchestrator.proposal.pauseEnded", "run_ended", ended),
    )),
    action(("resume_run", going), resume(going), ActionOutcome::Waits),
    settled(action(("resume_run", going), resume(going), ActionOutcome::Applied)),
    settled(action(
      ("resume_run", limited),
      resume(limited),
      refused("orchestrator.proposal.resumeStopped.tokens", "person_only", limited),
    )),
    release(ActionOutcome::Waits),
    settled(release(ActionOutcome::Applied)),
    settled(action(
      ("send_message", going),
      message(going),
      refused("orchestrator.proposal.messageChanged", "changed_since_proposed", going),
    )),
  ];
  cards.extend(moving_cards(&db, &runs, [going, limited, ended])?);
  let turns = vec![Turn {
    text: "Look after the runs while I'm away.",
    at: ago(40),
    parts: cards.into_iter().map(|card| Part::Action(Box::new(card))).collect(),
    end: TurnState::Answered,
  }];
  let chat = keep(&mut db, ChatId::generate(), connection, turns)?;
  elsewhere(&mut db)?;
  Ok(chat)
}

/// The words of every message card.
const TEXT: &str = "Also cover the 429 response: wait the Retry-After seconds before the next attempt.";

/// Messages that would move a run none of whose agents waits or works, waiting for the person: a follow-up, a
/// continuation, a resume, one that waits for a raised limit and one that runs the task again; `[going, limited,
/// ended]` are the runs the other cards picked.
fn moving_cards(
  db: &ProjectDb,
  runs: &[(Run, String)],
  [going, limited, ended]: [&(Run, String); 3],
) -> Result<Vec<ChatAction>, Box<dyn Error>> {
  let moving = |run: &(Run, String), then: MessageThen, sent_as: &str| {
    let change = Change::Message {
      run: run.0.id(),
      number: run.0.number().get(),
      text: TEXT.to_owned(),
      now: false,
      then: Some(then),
      sent_as: Some(sent_as.to_owned()),
    };
    action(("send_message", run), change, ActionOutcome::Waits)
  };
  let ready = pick_ready(db, runs)?;
  let stuck = pick(runs, |run| matches!(run.status(), RunStatus::WaitingForPerson(_)))?;
  Ok(vec![
    moving(ready, MessageThen::FollowUp, "follow_up"),
    moving(stuck, MessageThen::Continue, "continue"),
    // The seed has no paused run: the card reads its run as it stands.
    moving(going, MessageThen::Resume, "resume"),
    moving(limited, MessageThen::RaiseLimit, "raise_limit"),
    moving(ended, MessageThen::RunAgain, "run_again"),
  ])
}

/// A conversation, newer than the state's, that dealt with every task of the seed, so that the waits and run ends the
/// project's runner raises once the window opens land there, not between the state's cards (the conversation that
/// last dealt with a task gets them).
pub fn elsewhere(db: &mut ProjectDb) -> Result<ChatId, Box<dyn Error>> {
  let connection = db
    .orchestrator_settings()?
    .ok_or("the Orchestrator runs on the seed's connection")?
    .connection();
  let parts = db
    .tasks()?
    .into_iter()
    .map(|task| {
      let updated = ChatAction::new("update_task", Some(task.key().to_string()), ActionOutcome::Done);
      Part::Action(Box::new(updated))
    })
    .collect();
  let turn = Turn {
    text: "Where the project's waits and run ends land in this QA state.",
    at: Timestamp::now(),
    parts,
    end: TurnState::Answered,
  };
  keep(db, ChatId::generate(), connection, vec![turn])
}

/// The latest run of each of the seed's tasks that has one, with the task's key.
pub(super) fn latest_runs(db: &ProjectDb) -> Result<Vec<(Run, String)>, Box<dyn Error>> {
  let mut runs = Vec::new();
  for task in db.tasks()? {
    if let Some(Ok(run)) = db.runs_of_task(task.id())?.pop() {
      runs.push((run, task.key().to_string()));
    }
  }
  Ok(runs)
}

/// The first of `runs` that succeeded and was not merged: ready for review.
pub(super) fn pick_ready<'a>(db: &ProjectDb, runs: &'a [(Run, String)]) -> Result<&'a (Run, String), Box<dyn Error>> {
  for found in runs {
    if *found.0.status() == RunStatus::Succeeded && db.run_merge(found.0.id())?.is_none() {
      return Ok(found);
    }
  }
  Err("the tasks scenario has a run ready for review".into())
}

/// The first of `runs` that `wanted`.
fn pick(runs: &[(Run, String)], wanted: impl Fn(&Run) -> bool) -> Result<&(Run, String), Box<dyn Error>> {
  runs
    .iter()
    .find(|(run, _)| wanted(run))
    .ok_or_else(|| "the tasks scenario has such a run".into())
}

/// The action of `tool` on `run`'s task, about `change`, gone as `outcome`.
fn action((tool, run): (&str, &(Run, String)), change: Change, outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change,
    })),
    ..ChatAction::new(tool, Some(run.1.clone()), outcome)
  }
}

/// A release of the project's new starts, gone as `outcome`.
fn release(outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Release { task: None },
    })),
    ..ChatAction::new("release_queue", None, outcome)
  }
}

/// `action` settled by the person now.
fn settled(action: ChatAction) -> ChatAction {
  ChatAction {
    settled: Some(Timestamp::now()),
    ..action
  }
}
