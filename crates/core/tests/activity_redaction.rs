use std::sync::Arc;

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  connection::stream::{AgentEvent, CommandStatus, OutputStream},
  secret::SecretValue,
};

#[test]
fn activity_redaction_covers_commands_output_errors_and_json_without_changing_ids() {
  let value = "token\\path\"line\nend";
  let secrets = [SecretVariable::new(
    "TOKEN",
    Arc::new(SecretValue::new(value.into()).expect("secret")),
  )];
  let encoded = serde_json::to_string(value).expect("JSON");
  let text = format!("literal {value} encoded {encoded}");
  let mut events = [
    AgentEvent::CommandStarted {
      item: "item-1".into(),
      command: text.clone(),
    },
    AgentEvent::CommandFinished {
      item: "item-1".into(),
      command: text.clone(),
      output: text.clone(),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    },
    AgentEvent::Message { text: text.clone() },
    AgentEvent::Reasoning { text: text.clone() },
    AgentEvent::TurnFailed { message: text.clone() },
    AgentEvent::Raw {
      stream: OutputStream::Stdout,
      line: text,
    },
  ];
  for event in &mut events {
    event.redact(&|text| redact_secrets(text, &secrets));
    let serialized = serde_json::to_string(event).expect("serialize");
    assert!(!serialized.contains("token"));
    assert!(serialized.contains("<redacted>"));
  }
  assert!(matches!(&events[0], AgentEvent::CommandStarted { item, .. } if item == "item-1"));
  let mut session = AgentEvent::SessionStarted { session: value.into() };
  session.redact(&|text| redact_secrets(text, &secrets));
  assert_eq!(session, AgentEvent::SessionStarted { session: value.into() });
}
