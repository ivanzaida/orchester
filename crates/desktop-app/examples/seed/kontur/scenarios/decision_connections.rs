//! Frame 02 and frame 07's stored connection states for independent visual verification.
use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::{DecisionConnectionId, SecretId},
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
};

use crate::kontur::{Kontur, SeedError, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::connections::seed(kontur)?;
  let key = SecretId::generate();
  kontur.db.create_secret(&Secret::new(
    key,
    NewSecret {
      name: SecretName::new("TYPESAFE_API_KEY").expect("fixture secret name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Production,
    },
    kontur.now,
  ))?;
  let missing = ModelId::new("jev-preview").expect("fixture model");
  let fixtures = [
    (
      "jev · TypeSafe",
      "https://api.typesafe.ai/v1",
      DecisionCheckResult::Connected,
    ),
    (
      "jev · key refused",
      "https://api.typesafe.ai/v1",
      DecisionCheckResult::KeyRefused,
    ),
    (
      "Laya · this computer",
      "http://127.0.0.1:8090/v1",
      DecisionCheckResult::NothingAnswers,
    ),
    (
      "Other · decisions",
      "https://api.example.com/v1",
      DecisionCheckResult::NotSystemOneApi,
    ),
    (
      "jev · model missing",
      "https://api.typesafe.ai/v1",
      DecisionCheckResult::ModelGone { model: missing },
    ),
  ];
  for (label, url, result) in fixtures {
    let mut connection = DecisionConnection::new(
      DecisionConnectionId::generate(),
      name(label),
      DecisionServerUrl::new(url).expect("fixture URL"),
      (!url.contains("127.0.0.1")).then_some(key),
    )
    .with_models(vec![ModelId::new("jev-latest").expect("fixture model")]);
    connection.record_check(DecisionCheck { at: kontur.now, result });
    kontur.db.create_decision_connection(&connection)?;
    if label == "jev · TypeSafe" {
      kontur.db.save_auto_mode_settings(&AutoModeSettings {
        reviewer: Some(ReviewerChoice::decision(
          connection.id(),
          ModelId::new("jev-latest").expect("fixture model"),
          None,
        )),
      })?;
      test_result(kontur, &connection)?;
    }
  }
  Ok(())
}

pub fn missing(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let connection = kontur
    .db
    .decision_connections()?
    .into_iter()
    .find(|connection| connection.name().as_str() == "jev · model missing")
    .expect("seed wrote the missing-model connection");
  kontur.db.save_auto_mode_settings(&AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(
      connection.id(),
      ModelId::new("jev-preview").expect("fixture model"),
      None,
    )),
  })?;
  Ok(())
}

fn test_result(kontur: &mut Kontur, connection: &DecisionConnection) -> Result<(), SeedError> {
  use orchester_core::{
    id::IdempotencyKey,
    run::review::{DecisionTestReport, DecisionTestStatus, Probability, ThresholdFigures},
  };
  let report = DecisionTestReport {
    id: IdempotencyKey::generate(),
    connection: connection.id(),
    base_url: connection.base_url().clone(),
    key: connection.key(),
    model: connection.models()[0].clone(),
    started_at: kontur.now,
    status: DecisionTestStatus::Completed,
    requests: 9803,
    tested: 9803,
    previously_asked: Some(107),
    in_flight: 0,
    failed: 0,
    too_long: 0,
    median_ms: Some(182),
    input_tokens: 21_000_000,
    unknown_usage: 0,
    omitted_fields: vec![],
    thresholds: [
      (0.5, 9621, 182, 61),
      (0.8, 8852, 951, 29),
      (0.9, 8141, 1662, 19),
      (0.95, 7017, 2786, 10),
    ]
    .into_iter()
    .map(
      |(threshold, approves, asks_person, previously_asked_now_approved)| ThresholdFigures {
        threshold: Probability::new(threshold).expect("fixture threshold"),
        approves,
        asks_person,
        previously_asked_now_approved,
      },
    )
    .collect(),
  };
  kontur.db.begin_decision_test(&report)?;
  Ok(())
}
