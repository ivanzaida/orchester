//! A new profile on one of the person's API accounts (`Agents on accounts` C2 `t7s3I`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts"): the account is the app's, so it is on the rail of the
//! *Runs on* picker in a project that has no connection on it, with the models the person listed for it; looking at
//! it makes the project's connection, which is the account's copy, and the profile then runs on that.
//! `cargo test -p orchester-app --test agents local_hub_create_api`.

use orchester_app::{routes::HubFilter, test_support::signed_in::MemoryKeychain};
use orchester_core::{
  account::{AccountCli, ApiSetup},
  connection::BaseUrl,
  id::{ApiAccountId, SecretId},
  name::Name,
  resource::ModelId,
  secret::SecretValue,
};

use super::{harness::Agents, local_hub_create::show_form, local_hub_create_profile::RUNS_ON};

/// *DeepSeek*, an API account of the person's for Claude Code that serves `deepseek-chat`.
fn deepseek(agents: &Agents) -> ApiAccountId {
  let workspace = agents.window.launch.workspace().expect("workspace");
  workspace
    .add_api_account(
      &MemoryKeychain::default(),
      Name::new("DeepSeek").expect("name"),
      ApiSetup::cli(
        AccountCli::ClaudeCode,
        BaseUrl::new("https://api.deepseek.com/anthropic").expect("a base URL"),
        vec![ModelId::new("deepseek-chat").expect("a model id")],
      )
      .expect("a setup"),
      &SecretValue::new("sk-typed-by-the-test".to_owned()).expect("a key"),
    )
    .expect("added")
    .id()
}

#[test]
fn local_hub_create_api_a_profile_runs_on_the_persons_api_account() {
  let mut agents = Agents::new(true);
  let account = deepseek(&agents);
  show_form(&mut agents, HubFilter::Profiles);
  agents.set("team-new-profile-role-name-input", "Reviewer");
  agents.click(RUNS_ON);
  let menu = format!("{RUNS_ON}-menu");
  let item = format!("{menu}-connection-api-{account}");
  agents.until("the API account on the rail", |s| s.exists(&item));

  // Looking at it makes the project's connection on it: a copy of the account, under the account's id as its key's.
  agents.click(&item);
  let key = SecretId::from_uuid(account.as_uuid());
  agents.until("the project's connection on the account", |s| {
    s.listed()
      .connections
      .iter()
      .any(|connection| connection.api_provider().is_some_and(|provider| provider.key() == key))
  });
  let listed = agents.listed();
  let connection = listed
    .connections
    .iter()
    .find(|connection| connection.api_provider().is_some())
    .expect("made");
  assert_eq!(connection.name().as_str(), "Claude Code · DeepSeek");
  assert_eq!(
    agents.text(RUNS_ON),
    "Choose an account and a model",
    "looking at an account chooses nothing"
  );

  let model = format!("{menu}-model-0");
  agents.until("the account's model", |s| {
    s.exists(&model) && s.text(&model).contains("deepseek-chat")
  });
  agents.click(&model);
  agents.until("the field says it", |s| {
    s.text(RUNS_ON) == "Claude Code · DeepSeek · deepseek-chat"
  });
  agents.click(RUNS_ON);
  agents.until("the picker closed", |s| !s.exists(&menu));
  // Create waits for the project's connection on the account, then takes the press.
  agents.press_until("local-hub-new-create", "the profile", |s| {
    s.listed()
      .profiles
      .iter()
      .any(|entry| entry.profile.role().as_str() == "Reviewer")
  });
  let listed = agents.listed();
  let made = &listed
    .profiles
    .iter()
    .find(|entry| entry.profile.role().as_str() == "Reviewer")
    .expect("the profile")
    .profile;
  assert_eq!(made.connection(), connection.id());
  assert_eq!(made.model().as_str(), "deepseek-chat");
}
