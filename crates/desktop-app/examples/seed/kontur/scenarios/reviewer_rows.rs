//! Reviewer row fixtures for 03 and 04 a–e; recorded results only, with no server contacted.
use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionTest, DecisionThresholdFigures},
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice},
};
use orchester_workspace::project_settings::decision_reviewer::threshold;

use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::decision_connections::seed(kontur)?;
  for mut connection in kontur.db.decision_connections()? {
    if connection.name().as_str() == "Laya · this computer" {
      connection = connection.with_models(vec![ModelId::new("convaiinnovations/laya").expect("fixture model")]);
      connection.record_check(DecisionCheck {
        at: kontur.now,
        result: DecisionCheckResult::Connected,
      });
      kontur.db.save_decision_connection(&connection)?;
    } else if connection.name().as_str() == "jev · TypeSafe" {
      connection.record_test(DecisionTest {
        at: kontur.now,
        model: ModelId::new("jev-latest").expect("fixture model"),
        figures: vec![figure("0.80", 8823, 980), figure("0.90", 8141, 1662)],
      });
      kontur.db.save_decision_connection(&connection)?;
    } else {
      kontur.db.delete_decision_connection(connection.id())?;
    }
  }
  Ok(())
}

fn figure(value: &str, approves: u64, person: u64) -> DecisionThresholdFigures {
  DecisionThresholdFigures {
    threshold: threshold(value).expect("fixture threshold"),
    approves_on_its_own: approves,
    comes_to_person: person,
    approves_earlier_reviewer_asks: 0,
    requests_tested: approves + person,
    median_ms: 12,
    tokens: 500,
  }
}

pub fn local(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  choose_local(kontur, false)
}

pub fn unavailable(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  choose_local(kontur, true)
}

fn choose_local(kontur: &mut Kontur, unavailable: bool) -> Result<(), SeedError> {
  let mut connection = kontur
    .db
    .decision_connections()?
    .into_iter()
    .find(|c| c.is_on_this_computer())
    .expect("seed writes Laya");
  if unavailable {
    connection.record_check(DecisionCheck {
      at: kontur.now,
      result: DecisionCheckResult::NothingAnswers,
    });
    kontur.db.save_decision_connection(&connection)?;
  }
  kontur.db.save_auto_mode_settings(&AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      connection.models()[0].clone(),
      None,
    )),
  })?;
  Ok(())
}
