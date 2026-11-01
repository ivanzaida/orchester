//! The profile editor's *Browser* permission (`docs/design/browser.md`, "Permissions and policy"): a site that is not
//! a host is refused with core's reason, the browser needs network access, and turning it on or adding a site no
//! published site covers widens the profile. `cargo test -p orchester-workspace --test agents browser_`.
use orchester_core::{
  connection::AgentConnection,
  id::ResourceId,
  name::Name,
  permission::{
    AgentPermissions, BrowserPermission, CommandPermission, NetworkAccess, RepositoryAccess,
    browser::{AllowedSite, AllowedSites, BrowserAccess},
  },
  resource::{AgentProfile, PromptText, ProviderSettings},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::{
    data::{create_profile, publish, save_draft},
    permissions::{set_browser_access, set_network, set_sites, with_site, without_site},
    widening,
  },
  i18n::{FALLBACK_LOCALE, LOCALES},
};
use serde_json::Value;

use super::{codex, model, now, project_db};

fn sites(entries: &[&str]) -> AllowedSites {
  AllowedSites::new(
    entries
      .iter()
      .map(|entry| AllowedSite::new(entry).expect("a host"))
      .collect(),
  )
  .expect("distinct sites")
}

fn network_on() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::On,
  )
}

/// The words of `key` in the workspace's fallback locale.
fn words(key: &str) -> String {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("the fallback locale is shipped");
  let locale: Value = serde_json::from_str(contents).expect("the locale is JSON");
  key
    .split('.')
    .fold(&locale, |value, part| &value[part])
    .as_str()
    .unwrap_or_else(|| panic!("{key} is in {FALLBACK_LOCALE}.json"))
    .to_owned()
}

#[test]
fn browser_sites_refuse_what_is_not_a_host_with_its_reason() {
  let listed = sites(&["docs.rs"]);
  for (typed, key) in [
    (
      "https://staging.kontur.dev/login",
      "agents.permissions.siteInvalid.scheme",
    ),
    ("staging.kontur.dev/login", "agents.permissions.siteInvalid.path"),
    ("localhost:5173", "agents.permissions.siteInvalid.port"),
    ("docs rs", "agents.permissions.siteInvalid.spaces"),
    ("*.com", "agents.permissions.siteInvalid.tooBroad"),
    ("docs.*.rs", "agents.permissions.siteInvalid.wildcard"),
    ("   ", "agents.permissions.siteInvalid.blank"),
    ("DOCS.rs.", "agents.permissions.siteInvalid.duplicate"),
  ] {
    let refused = with_site(&listed, typed).expect_err(typed);
    assert_eq!(refused.key, key, "{typed}");
    assert!(!words(key).is_empty(), "{key} has words");
  }
  let scheme = with_site(&listed, " https://docs.rs ").expect_err("a URL");
  assert_eq!(scheme.args, [("site", "https://docs.rs".to_owned())]);
  assert!(words(scheme.key).contains("{{site}}"));
}

#[test]
fn browser_sites_are_added_in_order_and_removed() {
  let listed = with_site(&sites(&["docs.rs"]), "*.Example.com").expect("a wildcard of a domain");
  let shown: Vec<String> = listed.entries().iter().map(ToString::to_string).collect();
  assert_eq!(shown, ["docs.rs", "*.example.com"]);
  let docs = AllowedSite::new("docs.rs").expect("a host");
  let rest = without_site(&listed, &docs);
  assert_eq!(rest, sites(&["*.example.com"]));
}

#[test]
fn browser_needs_network_and_goes_off_with_it_keeping_its_sites() {
  let mut permissions = network_on();
  permissions.set_network(NetworkAccess::Off);
  let refused = set_browser_access(&mut permissions, BrowserAccess::On).expect_err("no network");
  assert_eq!(refused.key, "appMcp.errors.browserNeedsNetwork");
  assert_eq!(permissions.browser(), &BrowserPermission::off());

  set_network(&mut permissions, NetworkAccess::On);
  set_browser_access(&mut permissions, BrowserAccess::On).expect("network on");
  set_sites(&mut permissions, sites(&["docs.rs", "*.example.com"]));
  assert_eq!(permissions.browser().access(), BrowserAccess::On);

  set_network(&mut permissions, NetworkAccess::Off);
  assert_eq!(permissions.browser().access(), BrowserAccess::Off);
  assert_eq!(permissions.browser().sites(), &sites(&["docs.rs", "*.example.com"]));
}

/// A profile on `connection` with `permissions`.
fn profile(connection: &AgentConnection, permissions: AgentPermissions) -> AgentProfile {
  AgentProfile::new(
    Name::new("Builder").expect("name"),
    "Builds the change".into(),
    PromptText::new("Implement the approved plan.").expect("instructions"),
    connection.id(),
    model("gpt-5.5"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions)
}

/// What saving `permissions` over the latest published version of `id` widens, by key.
fn widens(
  db: &ProjectDb,
  id: ResourceId,
  connection: &AgentConnection,
  permissions: AgentPermissions,
) -> Vec<&'static str> {
  widening::reasons(db, Some(id), &profile(connection, permissions), connection.provider())
    .expect("reasons")
    .into_iter()
    .map(|reason| reason.key)
    .collect()
}

#[test]
fn browser_turned_on_or_a_new_site_widens_the_published_profile() {
  let (repo, mut db) = project_db();
  let codex = codex(&repo);
  db.create_connection(&codex).expect("store the connection");
  let id = create_profile(&mut db, &profile(&codex, network_on()), now()).expect("profile");
  publish(&mut db, id, now()).expect("v1");

  let mut on = network_on();
  set_browser_access(&mut on, BrowserAccess::On).expect("network on");
  assert_eq!(widens(&db, id, &codex, on.clone()), ["appMcp.widens.browser"]);
  set_sites(&mut on, sites(&["docs.rs"]));
  assert_eq!(
    widens(&db, id, &codex, on),
    ["appMcp.widens.browser", "appMcp.widens.browserSite"]
  );

  // Published with the browser on and *.example.com, a site under it widens nothing; another one does.
  let mut published = network_on();
  set_browser_access(&mut published, BrowserAccess::On).expect("network on");
  set_sites(&mut published, sites(&["*.example.com"]));
  save_draft(&mut db, id, profile(&codex, published.clone()), now()).expect("draft");
  publish(&mut db, id, now()).expect("v2");
  let mut covered = published.clone();
  set_sites(&mut covered, sites(&["*.example.com", "staging.example.com"]));
  assert!(widens(&db, id, &codex, covered).is_empty());
  let mut wider = published;
  set_sites(&mut wider, sites(&["*.example.com", "docs.rs"]));
  assert_eq!(widens(&db, id, &codex, wider), ["appMcp.widens.browserSite"]);
  assert!(!words("appMcp.widens.browser").is_empty() && !words("appMcp.widens.browserSite").is_empty());
}
