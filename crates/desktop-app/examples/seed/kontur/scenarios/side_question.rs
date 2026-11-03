//! `side-question` and `side-question-sent` (ORC-337, `Task page · redesign` 12b `gC8a7`, 12c `Y595T`): the `tasks`
//! scenario with a side question kept for KON-13's latest run, whose Builder waits at Implement; answered by the side
//! agent with three sources, and in `side-question-sent` passed on to Builder as a message. Open
//! `/tasks/KON-13/conversation` and press the composer's *Side question* button to show the pane.

use orchester_core::{
  id::SideQuestionId,
  run::side_question::{SideQuestion, SideQuestionText, SideReply, SideSource, SourceKind},
  store::RunStore,
};
use orchester_workspace::tasks::side_question::send_on_in;

use super::tasks;
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  answered(kontur).map(drop)
}

pub fn sent(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (task, id) = answered(kontur)?;
  send_on_in(&kontur.repo, (task, id))
    .map(drop)
    .map_err(|refused| SeedError::Invalid(format!("the side question was not sent: {refused:?}")))
}

/// The `tasks` project with an answered side question about KON-13's latest run.
fn answered(kontur: &mut Kontur) -> Result<(orchester_core::id::TaskId, SideQuestionId), SeedError> {
  tasks::project(kontur)?;
  let task = kontur.find("KON-13")?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .ok_or(SeedError::NotAtWork)?;
  let question = SideQuestionText::new(
    "Why did Builder change src/fixtures/retry.json? I thought this step was only about the sender.",
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  let id = SideQuestionId::generate();
  let asked = SideQuestion::asked(id, (task.id(), run.id()), question, kontur.ago(2));
  kontur.db.keep_side_question(&asked)?;
  let source = |kind, name: &str, detail: &str| SideSource {
    kind,
    name: name.to_owned(),
    detail: detail.to_owned(),
  };
  let reply = SideReply {
    text: "Builder's third test run failed because the fixture had no 30 s retry case. It added that case to \
           src/fixtures/retry.json so the test can run it. The retry change itself is only in src/sender.rs; the \
           fixture edit feeds the test and nothing else."
      .to_owned(),
    sources: vec![
      source(SourceKind::File, "src/fixtures/retry.json", "+12 \u{2212}0"),
      source(SourceKind::Command, "cargo test -p webhooks retry", "failed"),
      source(SourceKind::Message, "Builder", "message"),
    ],
  };
  kontur
    .db
    .end_side_question(id, Ok((&reply, "claude-haiku-5-5")), kontur.ago(2))?;
  Ok((task.id(), id))
}
