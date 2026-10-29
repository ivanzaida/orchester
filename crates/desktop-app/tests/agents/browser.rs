//! The profile's *Browser* row and its allowed sites (`08 · Profile · Browser permission` `hQfEh`, ORC-368): off
//! without network, on with its sites and what limits them, a site that is not a host refused in place, the
//! Connection enforcement line, and turning it on or adding a site asked about as a widening.
//! `cargo test -p orchester-app --test agents agents_browser_`.
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::ResourceId,
  permission::{
    AgentPermissions, BrowserPermission, CommandPermission, NetworkAccess, RepositoryAccess,
    browser::{AllowedSite, AllowedSites, BrowserAccess},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data::{self, ProfileChange};

use super::harness::Agents;

fn permissions(network: NetworkAccess, access: BrowserAccess, sites: &[&str]) -> AgentPermissions {
  let mut permissions = AgentPermissions::new(RepositoryAccess::ReadOnly, CommandPermission::AskBefore, network);
  let sites = sites
    .iter()
    .map(|site| AllowedSite::new(site).expect("a host"))
    .collect();
  permissions.set_browser(BrowserPermission::new(access, AllowedSites::new(sites).expect("sites")));
  permissions
}

/// A profile with `permissions` saved as its draft, published when `publish`.
fn profile_with(screen: &Agents, permissions: AgentPermissions, publish: bool) -> ResourceId {
  let id = screen.profile(false);
  let profile = screen
    .listed()
    .profile(id)
    .expect("profile")
    .profile
    .clone()
    .with_permissions(permissions);
  data::apply_in(&screen.root, ProfileChange::SaveDraft(id, profile)).expect("fixture draft");
  if publish {
    let mut db = ProjectDb::open(&screen.root).expect("db");
    data::publish(&mut db, id, Timestamp::now()).expect("published");
  }
  id
}

fn saved_browser(screen: &Agents, id: ResourceId) -> BrowserPermission {
  screen
    .listed()
    .profile(id)
    .expect("profile")
    .profile
    .permissions()
    .browser()
    .clone()
}

#[test]
fn agents_browser_row_stays_off_without_network_and_says_why() {
  let mut screen = Agents::new(true);
  let id = profile_with(
    &screen,
    permissions(NetworkAccess::Off, BrowserAccess::On, &["docs.rs"]),
    false,
  );
  screen.open(id);
  let row = screen.text("agent-browser-row");
  assert!(
    row.contains("Browser") && row.contains("Needs Network access") && row.contains("Off"),
    "{row}"
  );
  assert!(screen.text("agent-browser-reason").contains("Turn on Network access"));
  assert!(
    !screen.exists("agent-sites-row"),
    "no allowed sites while the browser is off"
  );
  screen.click("agent-browser");
  assert!(screen.text("agent-browser-row").contains("Off"), "the switch stays off");
  let enforcement = screen.text("agent-enforcement-browser");
  assert!(enforcement.contains("Off · no browser tools"), "{enforcement}");
  // Until Orchester's browser runs (P2), the section says so instead of showing a state it cannot have.
  let note = screen.text("agent-browser-note");
  assert!(
    note.contains("once Orchester’s browser runs") && note.contains("can’t watch"),
    "{note}"
  );
}

#[test]
fn agents_browser_on_lists_its_sites_and_what_limits_them() {
  let mut screen = Agents::new(true);
  let id = profile_with(
    &screen,
    permissions(NetworkAccess::On, BrowserAccess::On, &["docs.rs", "*.example.com"]),
    true,
  );
  screen.open(id);
  assert!(screen.text("agent-browser-row").contains("On"));
  assert!(!screen.exists("agent-browser-reason"));
  let sites = screen.text("agent-sites");
  assert!(sites.contains("docs.rs") && sites.contains("*.example.com"), "{sites}");
  let local = screen.text("agent-sites-local");
  assert!(
    local.contains("localhost, 127.0.0.1, [::1], *.localhost and *.test"),
    "{local}"
  );
  assert!(
    screen
      .text("agent-sites-reach")
      .contains("limit where the agent goes and acts, not what a page loads")
  );
  let enforcement = screen.text("agent-enforcement-browser");
  assert!(
    enforcement.contains("Orchester tools · 2 sites · others ask"),
    "{enforcement}"
  );
  assert!(enforcement.contains("Enforced"), "{enforcement}");

  // Turning network off turns the browser off with it; its sites are kept.
  let network_blocks = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  )
  .effective_network(AgentProvider::CodexCli, Platform::current())
    == NetworkAccess::Off;
  if network_blocks {
    screen.click("agent-network");
    assert!(screen.text("agent-browser-row").contains("Off"));
    assert!(screen.exists("agent-browser-reason") && !screen.exists("agent-sites-row"));
    screen.until("saved off", |s| saved_browser(s, id).access() == BrowserAccess::Off);
    assert_eq!(saved_browser(&screen, id).sites().entries().len(), 2);
  }
}

#[test]
fn agents_browser_refuses_a_site_that_is_not_a_host_in_place() {
  let mut screen = Agents::new(true);
  let id = profile_with(
    &screen,
    permissions(NetworkAccess::On, BrowserAccess::On, &["docs.rs", "*.example.com"]),
    true,
  );
  screen.open(id);
  for (typed, reason) in [
    ("https://staging.kontur.dev/login", "without its scheme (https://)"),
    ("staging.kontur.dev/login", "without a path"),
    ("localhost:5173", "without a port"),
    ("staging kontur.dev", "without spaces"),
    ("*.com", "would allow too many sites"),
  ] {
    screen.set("agent-site", typed);
    screen.click("agent-add-site");
    let error = screen.text("agent-site-error");
    assert!(error.contains(reason), "{typed}: {error}");
    assert_eq!(screen.value("agent-site"), typed, "the field keeps the text");
    assert!(!screen.exists("agent-site-2"), "{typed} was not added");
  }
  screen.set("agent-site", "staging.kontur.dev");
  screen.click("agent-add-site");
  assert!(!screen.exists("agent-site-error"));
  assert!(screen.text("agent-site-2").contains("staging.kontur.dev"));
  assert_eq!(screen.value("agent-site"), "");
  screen.click("agent-site-0-remove");
  assert!(!screen.text("agent-sites").contains("docs.rs"));
}

#[test]
fn agents_browser_turned_on_or_a_new_site_is_asked_about_in_its_section() {
  let mut screen = Agents::new(true);
  let id = profile_with(&screen, permissions(NetworkAccess::On, BrowserAccess::Off, &[]), true);
  screen.open(id);
  screen.click("agent-browser");
  screen.until("the widening question", |s| s.exists("agent-confirm-confirm"));
  let section = screen.text("agent-browser-section");
  assert!(
    section.contains("Allow these changes to the agent profile?"),
    "{section}"
  );
  assert!(
    section.contains("The browser, which the latest version does not give."),
    "{section}"
  );
  assert_eq!(
    saved_browser(&screen, id).access(),
    BrowserAccess::Off,
    "nothing saved until allowed"
  );
  screen.press_until("agent-confirm-confirm", "the browser saved on", |s| {
    saved_browser(s, id).access() == BrowserAccess::On
  });

  screen.set("agent-site", "docs.rs");
  screen.click("agent-add-site");
  screen.until("the site's question", |s| {
    s.exists("agent-confirm-confirm")
      && s
        .text("agent-browser-section")
        .contains("Allowed sites of the browser that the latest version does not list.")
  });
  assert!(saved_browser(&screen, id).sites().is_empty());
  screen.press_until("agent-confirm-confirm", "the site saved", |s| {
    saved_browser(s, id).sites().entries().len() == 1
  });
}
