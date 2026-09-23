use std::collections::BTreeMap;

use orchester_core::{
  id::ConnectionId,
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ProviderSettingsError},
};

fn builder() -> AgentProfile {
  let settings = ProviderSettings::new(BTreeMap::from([("reasoning_effort".to_owned(), "high".to_owned())]))
    .expect("valid settings");
  AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}} as {{agent.role}}.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.1-codex").expect("valid model"),
    settings,
  )
}

#[test]
fn agent_profile_keeps_connection_model_and_settings() {
  let profile = builder();
  assert_eq!(profile.role().as_str(), "Builder");
  assert_eq!(profile.model().as_str(), "gpt-5.1-codex");
  assert_eq!(
    profile.settings().iter().collect::<Vec<_>>(),
    [("reasoning_effort", "high")]
  );
  assert_eq!(profile.instructions().variables().len(), 2);
}

#[test]
fn agent_profile_model_id_is_a_catalog_token() {
  assert!(ModelId::new("claude-opus-4-1@20250805").is_ok());
  for invalid in ["", "gpt 5", "model\n", &"m".repeat(ModelId::MAX_CHARS + 1)] {
    assert!(ModelId::new(invalid).is_err(), "{invalid:?} should be rejected");
  }
}

#[test]
fn agent_profile_setting_names_must_be_tokens() {
  for invalid in ["", "reasoning effort"] {
    let settings = BTreeMap::from([(invalid.to_owned(), "high".to_owned())]);
    assert_eq!(
      ProviderSettings::new(settings),
      Err(ProviderSettingsError::InvalidName(invalid.to_owned()))
    );
  }
}

#[test]
fn agent_profile_role_instructions_reject_unknown_variables() {
  assert!(PromptText::new("You are {{agent.persona}}.").is_err());
}

#[test]
fn agent_profile_body_round_trips_and_revalidates_on_read() {
  let profile = builder();
  let json = serde_json::to_string(&profile).unwrap();
  assert_eq!(serde_json::from_str::<AgentProfile>(&json).unwrap(), profile);

  let blank_model = json.replace("gpt-5.1-codex", "");
  assert!(serde_json::from_str::<AgentProfile>(&blank_model).is_err());
  let blank_role = json.replace(r#""Builder""#, r#""  ""#);
  assert!(serde_json::from_str::<AgentProfile>(&blank_role).is_err());
}
