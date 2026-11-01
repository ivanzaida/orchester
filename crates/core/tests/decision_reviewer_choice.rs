//! Decision reviewer selection and its fail-closed resolution, independent of workspace storage and UI.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, Connection, DecisionConnection, DecisionServerUrl},
  id::{ConnectionId, DecisionConnectionId},
  name::Name,
  resource::ModelId,
  run::review::{
    AutoModeSettings, DecisionReviewerConnection, Probability, ReviewerChoice, ReviewerConnection,
    ReviewerConnectionId, ReviewerKind, ReviewerUnavailable, decision_reviewer_person_only, resolve_reviewer,
  },
  time::Timestamp,
};

fn model() -> ModelId {
  ModelId::new("system-one").expect("model")
}

fn decision(url: &str) -> DecisionConnection {
  DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision model").expect("name"),
    DecisionServerUrl::new(url).expect("base URL"),
    None,
  )
  .with_models(vec![model()])
}

fn language(provider: AgentProvider) -> Connection {
  AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Language reviewer").expect("name"),
    provider,
    std::env::current_exe().expect("absolute executable"),
    Timestamp::from_unix_millis(1),
  )
  .expect("CLI")
  .into_connection()
}

fn settings(connection: &DecisionConnection, threshold: Option<Probability>) -> AutoModeSettings {
  AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(connection.id(), model(), threshold)),
  }
}

#[test]
fn decision_reviewer_choice_resolves_its_connection_model_and_threshold() {
  let connection = decision("http://localhost:8000");
  let threshold = Probability::new(0.91).expect("probability");
  let settings = settings(&connection, Some(threshold));
  let states = [DecisionReviewerConnection {
    connection: &connection,
    connected: true,
  }];
  let resolved = resolve_reviewer(&settings, &[], &states).expect("decision reviewer");
  assert_eq!(resolved.connection, ReviewerConnection::Decision(&connection));
  assert_eq!(
    resolved.connection.id(),
    ReviewerConnectionId::Decision {
      decision: connection.id()
    }
  );
  assert_eq!(resolved.kind, ReviewerKind::Decision);
  assert_eq!(
    resolved.kind.provider(),
    None,
    "decision reviews are not spent on an agent provider"
  );
  assert_eq!(resolved.model, model());
  assert_eq!(resolved.threshold, Some(threshold));
  assert_eq!(resolved.effort, None);
  assert!(!resolved.default);
}

#[test]
fn decision_reviewer_choice_defaults_threshold_on_construction_and_legacy_deserialization() {
  let connection = decision("http://127.0.0.1:8000");
  let expected = Some(Probability::new(0.80).expect("probability"));
  let chosen = settings(&connection, None);
  assert_eq!(chosen.reviewer.as_ref().expect("choice").threshold, expected);
  let choice: ReviewerChoice = serde_json::from_value(serde_json::json!({
    "connection": { "decision": connection.id() },
    "model": model(),
    "effort": "high"
  }))
  .expect("threshold omitted");
  let states = [DecisionReviewerConnection {
    connection: &connection,
    connected: true,
  }];
  let resolved = resolve_reviewer(&AutoModeSettings { reviewer: Some(choice) }, &[], &states)
    .expect("missing threshold defaults at resolution");
  assert_eq!(resolved.threshold, expected);
  assert_eq!(resolved.effort, None);
  let serialized = serde_json::to_value(chosen.reviewer.expect("choice")).expect("serialize");
  assert_eq!(serialized["threshold"], 0.80);
  for value in [
    serde_json::json!(-0.01),
    serde_json::json!(1.01),
    serde_json::json!("NaN"),
  ] {
    let invalid = serde_json::json!({
      "connection": { "decision": connection.id() }, "model": model(), "threshold": value
    });
    assert!(serde_json::from_value::<ReviewerChoice>(invalid).is_err());
  }
}

#[test]
fn decision_reviewer_choice_unavailable_reasons_never_fall_back() {
  let connection = decision("http://localhost:8000");
  let settings = settings(&connection, None);
  let languages = [language(AgentProvider::ClaudeCodeCli)];
  assert_eq!(
    resolve_reviewer(&settings, &languages, &[]),
    Err(ReviewerUnavailable::ConnectionGone)
  );
  let unrelated = decision("https://other.example");
  let states = [DecisionReviewerConnection {
    connection: &unrelated,
    connected: true,
  }];
  assert_eq!(
    resolve_reviewer(&settings, &languages, &states),
    Err(ReviewerUnavailable::ConnectionGone)
  );
  let states = [DecisionReviewerConnection {
    connection: &connection,
    connected: false,
  }];
  assert_eq!(
    resolve_reviewer(&settings, &languages, &states),
    Err(ReviewerUnavailable::NotConnected)
  );
  let mut changed = connection.clone();
  changed.record_models(vec![ModelId::new("replacement").expect("model")]);
  let states = [DecisionReviewerConnection {
    connection: &changed,
    connected: true,
  }];
  assert_eq!(
    resolve_reviewer(&settings, &languages, &states),
    Err(ReviewerUnavailable::ModelGone)
  );
  changed.record_models(vec![]);
  let states = [DecisionReviewerConnection {
    connection: &changed,
    connected: true,
  }];
  assert_eq!(
    resolve_reviewer(&settings, &languages, &states),
    Err(ReviewerUnavailable::ModelGone)
  );
}

#[test]
fn decision_reviewer_choice_off_this_computer_is_a_persons_alone() {
  for host in ["localhost", "LOCALHOST", "127.0.0.1", "[::1]"] {
    assert!(
      !decision_reviewer_person_only(&decision(&format!("http://{host}:8000"))),
      "{host}"
    );
  }
  for host in [
    "0.0.0.0",
    "127.0.0.2",
    "192.168.1.2",
    "model.example",
    "localhost.example",
  ] {
    assert!(
      decision_reviewer_person_only(&decision(&format!("https://{host}:8000"))),
      "{host}"
    );
  }
}

#[test]
fn decision_reviewer_choice_is_never_default() {
  let connection = decision("http://localhost:8000");
  let states = [DecisionReviewerConnection {
    connection: &connection,
    connected: true,
  }];
  assert_eq!(
    resolve_reviewer(&AutoModeSettings::default(), &[], &states),
    Err(ReviewerUnavailable::NoConnection)
  );
  let languages = [language(AgentProvider::ClaudeCodeCli)];
  let resolved = resolve_reviewer(&AutoModeSettings::default(), &languages, &states).expect("language default");
  assert_eq!(resolved.kind, ReviewerKind::ClaudeCode);
  assert!(resolved.default);
  assert_eq!(resolved.threshold, None);
}

#[test]
fn decision_reviewer_choice_preserves_language_settings_and_effort() {
  for provider in [AgentProvider::ClaudeCodeCli, AgentProvider::CodexCli] {
    let connection = language(provider);
    let choice: ReviewerChoice = serde_json::from_value(serde_json::json!({
      "connection": connection.id(), "model": "chosen-model", "effort": "high"
    }))
    .expect("old language settings");
    assert_eq!(choice.connection, connection.id().into());
    assert_eq!(choice.threshold, None);
    let serialized = serde_json::to_value(&choice).expect("serialize");
    assert_eq!(serialized["connection"], connection.id().to_string());
    assert!(serialized.get("threshold").is_none());
    let mut choice = choice;
    choice.threshold = Some(Probability::new(0.95).expect("probability"));
    let settings = AutoModeSettings { reviewer: Some(choice) };
    let languages = [connection];
    let resolved = resolve_reviewer(&settings, &languages, &[]).expect("language reviewer");
    assert_eq!(resolved.model.as_str(), "chosen-model");
    assert_eq!(resolved.effort.as_deref(), Some("high"));
    assert_eq!(resolved.threshold, None, "threshold never applies to a language model");
    assert!(!resolved.default);
  }
}
