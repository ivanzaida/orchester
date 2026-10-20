//! `resolve`: a conversation where the Orchestrator's agent proposes sending a run ready for review back to its
//! writing step to resolve its conflicts with the base (`continue_run`'s `resolve_conflicts`,
//! `Orchestrator · proposals in automatic turns · states` `sXEjJ`), seeded as the chat keeps it and waiting for the
//! person: its Run, Choice, Conflicting files and a Note long enough to wrap, the conflict's own words then the chat's.

use std::error::Error;

use orchester_core::{
  id::ChatId,
  orchestrator::{
    ActionOutcome, ChatAction, TurnState,
    change::{Change, ContinueChoice, ProposedChange, StepName},
  },
};
use orchester_store::ProjectDb;

use super::{
  data::{Part, Turn, ago, keep},
  proposals::{elsewhere, latest_runs, pick_ready},
};

/// What the chat adds for the agent.
const NOTE: &str = "Keep master's split of the settings into delivery.rs and retry.rs; move retry_budget into \
                    retry.rs and keep the defaults master chose. Run the webhook tests once the merge is committed.";

/// The conversation, kept in the project at `prepared`.
pub fn conversation(prepared: &super::data::Prepared) -> Result<ChatId, Box<dyn Error>> {
  let mut db = ProjectDb::open(&prepared.repo)?;
  let connection = db
    .orchestrator_settings()?
    .ok_or("the Orchestrator runs on the seed's connection")?
    .connection();
  let runs = latest_runs(&db)?;
  let (run, key) = pick_ready(&db, &runs)?;
  let choice = ContinueChoice::ResolveConflicts {
    step: "implement".to_owned(),
    to: Some(StepName {
      place: 1,
      name: "Implement".to_owned(),
    }),
    base: "master".to_owned(),
    files: vec![
      "crates/desktop-app/src/screens/orchestrator/change/facts.rs".to_owned(),
      "crates/desktop-app/assets/locales/en/orchestrator.json".to_owned(),
    ],
    note: Some(NOTE.to_owned()),
  };
  let action = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Continue {
        run: run.id(),
        number: run.number().get(),
        attempt: 1,
        step: None,
        why: None,
        choice,
      },
    })),
    ..ChatAction::new("continue_run", Some(key.clone()), ActionOutcome::Waits)
  };
  let turns = vec![Turn {
    text: "Its merge stopped at conflicts; send it back to resolve them.",
    at: ago(10),
    parts: vec![Part::Action(Box::new(action))],
    end: TurnState::Answered,
  }];
  let chat = keep(&mut db, ChatId::generate(), connection, turns)?;
  elsewhere(&mut db)?;
  Ok(chat)
}
