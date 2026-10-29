//! Allowed sites: what an entry may be, which hosts it matches, the local hosts, and how the list is stored.

use orchester_core::permission::{
  AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
  browser::{
    AllowedSite, AllowedSiteError, AllowedSites, AllowedSitesError, BrowserAccess, BrowserPermission, SiteHost,
  },
};

fn site(text: &str) -> AllowedSite {
  AllowedSite::new(text).unwrap_or_else(|error| panic!("{text} is a site: {error}"))
}

fn host(text: &str) -> SiteHost {
  SiteHost::new(text).unwrap_or_else(|| panic!("{text} is a host"))
}

#[test]
fn browser_site_takes_a_host_or_a_domain_with_its_hosts() {
  assert_eq!(site("docs.rs"), AllowedSite::Host(host("docs.rs")));
  assert_eq!(
    site("  Docs.RS. ").to_string(),
    "docs.rs",
    "trimmed, lower case, no trailing dot"
  );
  assert_eq!(site("*.example.com"), AllowedSite::Under(host("example.com")));
  assert_eq!(site("*.example.com").to_string(), "*.example.com");
  assert_eq!(site("192.168.1.20").to_string(), "192.168.1.20");
  assert_eq!(site("[::1]").to_string(), "[::1]");
  assert_eq!(
    site("[0:0:0:0:0:0:0:1]").to_string(),
    "[::1]",
    "an address in its short form"
  );
  assert_eq!(site("xn--bcher-kva.example").to_string(), "xn--bcher-kva.example");
}

#[test]
fn browser_site_refuses_what_is_not_a_host_with_its_reason() {
  for (text, error) in [
    ("", AllowedSiteError::Blank),
    ("   ", AllowedSiteError::Blank),
    ("docs .rs", AllowedSiteError::Spaces),
    ("https://staging.kontur.dev/login", AllowedSiteError::Scheme),
    ("https://docs.rs", AllowedSiteError::Scheme),
    ("mailto:someone", AllowedSiteError::Scheme),
    ("docs.rs/std", AllowedSiteError::Path),
    ("docs.rs?q=1", AllowedSiteError::Path),
    ("docs.rs#top", AllowedSiteError::Path),
    ("localhost:5173", AllowedSiteError::Port),
    ("[::1]:8080", AllowedSiteError::Port),
    ("user@docs.rs", AllowedSiteError::Credentials),
    ("docs.*.rs", AllowedSiteError::Wildcard),
    ("*", AllowedSiteError::Wildcard),
    ("*.*.example.com", AllowedSiteError::Wildcard),
    ("*.10.0.0.1", AllowedSiteError::Wildcard),
    ("*.com", AllowedSiteError::TooBroad("com".to_owned())),
    ("bücher.example", AllowedSiteError::NotAscii),
    ("-docs.rs", AllowedSiteError::NotAHost("-docs.rs".to_owned())),
    ("docs..rs", AllowedSiteError::NotAHost("docs..rs".to_owned())),
    (
      "under_score.dev",
      AllowedSiteError::NotAHost("under_score.dev".to_owned()),
    ),
    (
      "[not-an-address]",
      AllowedSiteError::NotAHost("[not-an-address]".to_owned()),
    ),
  ] {
    assert_eq!(AllowedSite::new(text), Err(error), "{text:?}");
  }
  let label = "a".repeat(64);
  assert!(
    AllowedSite::new(&format!("{label}.dev")).is_err(),
    "a label is at most 63 characters"
  );
}

#[test]
fn browser_site_matches_its_host_or_the_hosts_under_its_domain() {
  let docs = site("docs.rs");
  assert!(docs.matches(&host("docs.rs")));
  assert!(
    docs.matches(&host("DOCS.rs.")),
    "hosts compare in lower case without a trailing dot"
  );
  assert!(!docs.matches(&host("www.docs.rs")), "a host is only itself");
  let example = site("*.example.com");
  assert!(example.matches(&host("staging.example.com")));
  assert!(example.matches(&host("a.b.example.com")), "at any depth");
  assert!(!example.matches(&host("example.com")), "not the domain itself");
  assert!(!example.matches(&host("badexample.com")), "a whole label, not a suffix");
  assert!(!example.matches(&host("example.com.evil.dev")));
}

#[test]
fn browser_site_covers_what_it_allows() {
  assert!(site("*.example.com").covers(&site("staging.example.com")));
  assert!(site("*.example.com").covers(&site("*.eu.example.com")));
  assert!(site("*.example.com").covers(&site("*.example.com")));
  assert!(!site("*.example.com").covers(&site("example.com")));
  assert!(!site("staging.example.com").covers(&site("*.example.com")));
  assert!(site("docs.rs").covers(&site("docs.rs")));
}

#[test]
fn browser_local_development_hosts_are_always_allowed() {
  let off_list = BrowserPermission::new(BrowserAccess::On, AllowedSites::none());
  for local in [
    "localhost",
    "127.0.0.1",
    "[::1]",
    "app.localhost",
    "kontur.test",
    "api.kontur.test",
  ] {
    assert!(host(local).is_local(), "{local}");
    assert!(off_list.allows_site(&host(local)), "{local}");
  }
  for other in ["localhost.dev", "127.0.0.2", "test", "testing.dev", "docs.rs"] {
    assert!(!off_list.allows_site(&host(other)), "{other}");
  }
  let listed = BrowserPermission::new(
    BrowserAccess::On,
    AllowedSites::new(vec![site("docs.rs"), site("*.example.com")]).expect("two sites"),
  );
  assert!(listed.allows_site(&host("docs.rs")));
  assert!(listed.allows_site(&host("staging.example.com")));
  assert!(!listed.allows_site(&host("github.com")));
}

#[test]
fn browser_sites_list_each_site_once_and_at_most_fifty() {
  assert_eq!(
    AllowedSites::new(vec![site("docs.rs"), site("DOCS.rs")]),
    Err(AllowedSitesError::Duplicate(site("docs.rs")))
  );
  let many: Vec<AllowedSite> = (0..=AllowedSites::MAX_ENTRIES)
    .map(|n| site(&format!("s{n}.dev")))
    .collect();
  assert_eq!(AllowedSites::new(many), Err(AllowedSitesError::TooMany));
}

#[test]
fn browser_permission_is_left_out_of_the_stored_form_while_unset() {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  );
  let before = serde_json::to_value(&permissions).expect("permissions serialize");
  assert!(before.get("browser").is_none(), "{before}");
  let stored: AgentPermissions = serde_json::from_value(before).expect("permissions saved before the browser read");
  assert_eq!(stored.browser(), &BrowserPermission::off());
  permissions.set_browser(BrowserPermission::new(
    BrowserAccess::On,
    AllowedSites::new(vec![site("*.example.com")]).expect("a site"),
  ));
  let on = serde_json::to_value(&permissions).expect("permissions serialize");
  assert_eq!(
    on["browser"],
    serde_json::json!({"access": "on", "sites": ["*.example.com"]})
  );
  let read: AgentPermissions = serde_json::from_value(on).expect("the browser reads back");
  assert_eq!(read, permissions);
  let invalid = serde_json::json!({"access": "on", "sites": ["https://docs.rs"]});
  assert!(
    serde_json::from_value::<BrowserPermission>(invalid).is_err(),
    "a stored site is checked"
  );
}
