use std::collections::BTreeMap;

use orchester_core::{
  connection::mcp::{McpSelectionError, McpServerSelection},
  id::ConnectionId,
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ProviderSettingsError, ResourceBody},
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

#[test]
fn agent_profile_keeps_its_permissions() {
  let edit = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  let profile = builder().with_permissions(edit.clone());
  assert_eq!(profile.permissions(), &edit);
  let json = serde_json::to_string(&profile).unwrap();
  assert_eq!(serde_json::from_str::<AgentProfile>(&json).unwrap(), profile);
  assert_eq!(builder().permissions(), &AgentPermissions::before_permissions());
}

#[test]
fn agent_profile_schema_1_bodies_get_the_permissions_before_permissions() {
  let original = builder();
  let mut body = serde_json::to_value(&original).unwrap();
  let fields = body.as_object_mut().unwrap();
  fields.remove("permissions");
  fields.remove("mcp_servers");
  assert!(serde_json::from_value::<AgentProfile>(body.clone()).is_err());
  let upgraded = AgentProfile::upgrade(1, body.clone()).expect("schema 1 upgrades");
  let profile: AgentProfile = serde_json::from_value(upgraded).unwrap();
  assert_eq!(profile, original);
  assert_eq!(AgentProfile::SCHEMA, 3);
  assert!(AgentProfile::upgrade(0, body).is_none());
}

#[test]
fn agent_profile_schema_2_bodies_get_no_mcp_servers() {
  let original = builder();
  let mut body = serde_json::to_value(&original).unwrap();
  body.as_object_mut().unwrap().remove("mcp_servers");
  assert!(serde_json::from_value::<AgentProfile>(body.clone()).is_err());
  let upgraded = AgentProfile::upgrade(2, body).expect("schema 2 upgrades");
  let profile: AgentProfile = serde_json::from_value(upgraded).unwrap();
  assert_eq!(profile, original);
  assert!(profile.mcp_servers().is_empty());
}

#[test]
fn agent_profile_keeps_its_mcp_servers() {
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let servers = McpServerSelection::new(vec![first, second]).expect("two servers");
  let profile = builder().with_mcp_servers(servers.clone());
  let json = serde_json::to_string(&profile).unwrap();
  let read: AgentProfile = serde_json::from_str(&json).unwrap();
  assert_eq!(read.mcp_servers(), &servers);
  assert_eq!(
    McpServerSelection::new(vec![first, first]),
    Err(McpSelectionError::Duplicate(first))
  );
  let toggled = servers.toggled(first).expect("fewer servers");
  assert_eq!(toggled.ids(), &[second]);
  assert_eq!(toggled.toggled(first).expect("one more").ids(), &[second, first]);
  let tampered = json.replace(&second.to_string(), &first.to_string());
  assert!(serde_json::from_str::<AgentProfile>(&tampered).is_err());
}
