//! Agents QA states of the profile's Browser permission (`08 · Profile · Browser permission` `hQfEh`): the agents
//! scenario with three published Claude Code profiles, *Browser on* (network on, two allowed sites), *Browser off*
//! (network on) and *Browser without network*.
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::ConnectionId,
  permission::{
    AgentPermissions, AllowedCommands, BrowserPermission, CommandPermission, CommandPrefix, NetworkAccess,
    RepositoryAccess,
    browser::{AllowedSite, AllowedSites, BrowserAccess},
  },
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug},
  store::{ConnectionStore, ResourceStore},
};

use crate::kontur::{Kontur, SeedError, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::agents::seed(kontur)?;
  let claude = kontur
    .db
    .connections()?
    .into_iter()
    .filter_map(|stored| AgentConnection::try_from(stored).ok())
    .find(|connection| connection.provider() == AgentProvider::ClaudeCodeCli)
    .ok_or_else(|| SeedError::Invalid("the Browser fixture needs Claude".into()))?
    .id();
  let sites = AllowedSites::new(vec![site("docs.rs"), site("*.example.com")]).expect("fixture sites");
  for (title, network, browser) in [
    (
      "Browser on",
      NetworkAccess::On,
      BrowserPermission::new(BrowserAccess::On, sites),
    ),
    ("Browser off", NetworkAccess::On, BrowserPermission::off()),
    ("Browser without network", NetworkAccess::Off, BrowserPermission::off()),
  ] {
    let commands = AllowedCommands::new(vec![
      CommandPrefix::new("cargo test").expect("fixture prefix"),
      CommandPrefix::new("pnpm test").expect("fixture prefix"),
    ])
    .expect("fixture command list");
    let mut permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::Allowed(commands), network);
    permissions.set_browser(browser);
    publish(kontur, title, claude, permissions)?;
  }
  Ok(())
}

fn site(text: &str) -> AllowedSite {
  AllowedSite::new(text).expect("fixture site")
}

fn publish(
  kontur: &mut Kontur,
  title: &str,
  connection: ConnectionId,
  permissions: AgentPermissions,
) -> Result<(), SeedError> {
  let body = AgentProfile::new(
    name(title),
    "Checks the change in the browser.".into(),
    PromptText::new("Open the app the run builds and check the change in it.").expect("fixture prompt"),
    connection,
    ModelId::new("claude-sonnet-5-5").expect("fixture model"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions);
  let resource = kontur.db.create_resource(
    Slug::from_name(title).expect("fixture slug"),
    name(title),
    &body,
    kontur.now,
  )?;
  kontur.db.publish_draft::<AgentProfile>(resource.id(), kontur.now)?;
  Ok(())
}
