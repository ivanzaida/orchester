//! Network access Off, Hosts(list) or On (`docs/architecture/server.md`, row 75): which hosts a pattern accepts, the
//! list's rules, the order Off < Hosts < On, what widens for the person gate and for a teammate's version, and the
//! stored form, which still reads `"off"` and `"on"`. Run with
//! `cargo test -p orchester-core --test network_hosts network_hosts`.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentProvider, invocation::Platform, mcp::McpServerSelection},
  id::ConnectionId,
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, HostPattern, HostPatternError, NetworkAccess, NetworkHosts,
    NetworkHostsError, RepositoryAccess,
    capability::{PermissionOption, PermissionProblem, Unavailable},
    widening::Widening,
  },
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
  sharing::{VersionWidening, widens},
};
use serde_json::json;

fn host(pattern: &str) -> HostPattern {
  HostPattern::new(pattern).expect("a host pattern")
}

fn hosts(patterns: &[&str]) -> NetworkAccess {
  NetworkAccess::Hosts(NetworkHosts::new(patterns.iter().map(|pattern| host(pattern)).collect()).expect("a host list"))
}

fn permissions(network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    network,
  )
}

fn profile(network: NetworkAccess) -> AgentProfile {
  AgentProfile::new(
    Name::new("Researcher").expect("valid role"),
    "Looks things up.".into(),
    PromptText::new("You research {{task.title}}.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::new(BTreeMap::new()).expect("no settings"),
  )
  .with_permissions(permissions(network))
  .with_mcp_servers(McpServerSelection::none())
}

#[test]
fn network_hosts_patterns_are_lowercased_names_addresses_or_wildcard_domains() {
  assert_eq!(host("  API.Example.com ").as_str(), "api.example.com");
  assert_eq!(host("*.example.org").as_str(), "*.example.org");
  assert_eq!(host("10.0.0.7").as_str(), "10.0.0.7");
  assert_eq!(host("localhost").as_str(), "localhost");
  for (pattern, error) in [
    ("", HostPatternError::Blank),
    ("   ", HostPatternError::Blank),
    ("https://api.example.com", HostPatternError::Scheme),
    ("api.example.com/v1", HostPatternError::Path),
    ("api.example.com:443", HostPatternError::Port),
    ("api example.com", HostPatternError::ForbiddenCharacter(' ')),
    ("api_1.example.com", HostPatternError::ForbiddenCharacter('_')),
    ("api.*.example.com", HostPatternError::ForbiddenCharacter('*')),
    ("*", HostPatternError::ForbiddenCharacter('*')),
    ("user@example.com", HostPatternError::ForbiddenCharacter('@')),
    ("*.", HostPatternError::Label(String::new())),
    ("api..example.com", HostPatternError::Label(String::new())),
    ("example.com.", HostPatternError::Label(String::new())),
    ("-api.example.com", HostPatternError::Label("-api".to_owned())),
    ("api-.example.com", HostPatternError::Label("api-".to_owned())),
  ] {
    assert_eq!(HostPattern::new(pattern), Err(error), "`{pattern}`");
  }
  let long_label = format!("{}.example.com", "a".repeat(64));
  assert!(matches!(HostPattern::new(&long_label), Err(HostPatternError::Label(_))));
  let long_name = vec!["a".repeat(63); 4].join(".");
  assert_eq!(long_name.len(), 255);
  assert_eq!(HostPattern::new(&long_name), Err(HostPatternError::TooLong));
}

#[test]
fn network_hosts_lists_are_sorted_without_duplicates_and_never_empty() {
  let list = NetworkHosts::new(vec![
    host("b.example.com"),
    host("A.example.com"),
    host("a.example.com"),
  ])
  .expect("a host list");
  assert_eq!(list.entries(), [host("a.example.com"), host("b.example.com")]);
  assert_eq!(NetworkHosts::new(Vec::new()), Err(NetworkHostsError::Empty));
  let many = |count: usize| {
    (0..count)
      .map(|n| host(&format!("h{n}.example.com")))
      .collect::<Vec<_>>()
  };
  assert!(NetworkHosts::new(many(NetworkHosts::MAX_ENTRIES)).is_ok());
  assert_eq!(
    NetworkHosts::new(many(NetworkHosts::MAX_ENTRIES + 1)),
    Err(NetworkHostsError::TooMany)
  );
}

#[test]
fn network_hosts_are_ordered_off_hosts_on() {
  let list = hosts(&["api.example.com"]);
  let order = [NetworkAccess::Off, list, NetworkAccess::On];
  for (lower, below) in order.iter().enumerate() {
    for (higher, above) in order.iter().enumerate() {
      assert_eq!(above.covers(below), higher >= lower, "{above:?} covers {below:?}");
    }
  }
}

#[test]
fn network_hosts_a_list_covers_another_when_it_covers_each_of_its_hosts() {
  let wildcard = hosts(&["*.example.org"]);
  assert!(wildcard.covers(&hosts(&["api.example.org", "a.b.example.org"])));
  assert!(wildcard.covers(&hosts(&["*.api.example.org"])), "a deeper wildcard");
  assert!(wildcard.covers(&wildcard));
  assert!(
    !wildcard.covers(&hosts(&["example.org"])),
    "the domain itself is not a subdomain"
  );
  assert!(!wildcard.covers(&hosts(&["badexample.org"])), "only whole labels");
  assert!(!wildcard.covers(&hosts(&["api.example.org", "api.example.com"])));
  let one = hosts(&["api.example.org"]);
  assert!(!one.covers(&wildcard), "a host does not cover its domain's wildcard");
  assert!(hosts(&["api.example.org", "cdn.example.org"]).covers(&one));
  assert!(!one.covers(&hosts(&["api.example.org", "cdn.example.org"])));
}

#[test]
fn network_hosts_widen_up_the_order_and_by_a_host_not_covered() {
  let widened = |before: NetworkAccess, after: NetworkAccess| {
    permissions(after).widenings_from(&permissions(before)) == [Widening::Network]
  };
  let api = || hosts(&["api.example.org"]);
  assert!(widened(NetworkAccess::Off, api()), "off to a list");
  assert!(widened(api(), NetworkAccess::On), "a list to on");
  assert!(
    widened(api(), hosts(&["api.example.org", "cdn.example.org"])),
    "a host added"
  );
  assert!(
    widened(api(), hosts(&["cdn.example.org"])),
    "a host in place of another"
  );
  assert!(widened(api(), hosts(&["*.example.org"])), "a wildcard over a host");
  let narrows =
    |before: NetworkAccess, after: NetworkAccess| permissions(after).widenings_from(&permissions(before)).is_empty();
  assert!(narrows(NetworkAccess::On, api()), "on to a list");
  assert!(narrows(api(), NetworkAccess::Off), "a list to off");
  assert!(narrows(hosts(&["*.example.org"]), api()), "a host under the wildcard");
  assert!(
    narrows(hosts(&["api.example.org", "cdn.example.org"]), api()),
    "a host dropped"
  );
  assert!(narrows(api(), api()));
}

#[test]
fn network_hosts_a_teammates_version_with_a_wider_list_waits_for_the_person() {
  let network = VersionWidening::Permissions(Widening::Network);
  let api = profile(hosts(&["api.example.org"]));
  assert_eq!(
    widens(Some(&api), &profile(hosts(&["api.example.org", "cdn.example.org"]))),
    std::slice::from_ref(&network)
  );
  assert_eq!(
    widens(Some(&api), &profile(NetworkAccess::On)),
    std::slice::from_ref(&network)
  );
  assert_eq!(widens(None, &api), [network], "a first version with a list");
  assert!(widens(Some(&profile(NetworkAccess::On)), &api).is_empty());
  assert!(widens(Some(&api), &profile(NetworkAccess::Off)).is_empty());
}

#[test]
fn network_hosts_bodies_store_off_hosts_and_on_as_before() {
  let stored = |network: NetworkAccess| serde_json::to_value(permissions(network)).expect("serializable");
  assert_eq!(stored(NetworkAccess::Off)["network"], json!("off"));
  assert_eq!(stored(NetworkAccess::On)["network"], json!("on"));
  let list = hosts(&["*.example.org", "api.example.com"]);
  assert_eq!(
    stored(list.clone())["network"],
    json!({ "hosts": ["*.example.org", "api.example.com"] })
  );
  let read = |network: serde_json::Value| {
    let mut body = stored(NetworkAccess::Off);
    body["network"] = network;
    serde_json::from_value::<AgentPermissions>(body).map(|permissions| permissions.network().clone())
  };
  assert_eq!(read(json!("off")).expect("an old body"), NetworkAccess::Off);
  assert_eq!(read(json!("on")).expect("an old body"), NetworkAccess::On);
  assert_eq!(
    read(json!({ "hosts": ["API.example.com", "*.example.org"] })).expect("a list"),
    list
  );
  assert!(read(json!({ "hosts": [] })).is_err(), "an empty list");
  assert!(read(json!({ "hosts": ["https://example.com"] })).is_err(), "a scheme");
}

#[test]
fn network_hosts_run_only_where_network_off_is_enforced_and_are_never_reset_to_on() {
  let list = permissions(hosts(&["api.example.org"]));
  let mut confined = list.clone();
  confined.set_commands(CommandPermission::AnyInWorktree);
  let enforced = confined
    .enforced_on(AgentProvider::CodexCli, Platform::MacOs)
    .expect("network off is enforced there");
  assert_eq!(*enforced.permissions().network(), hosts(&["api.example.org"]));
  assert_eq!(enforced.network_stays_on(), None);
  for provider in [AgentProvider::ClaudeCodeCli, AgentProvider::AnthropicApi] {
    assert_eq!(
      list.enforced_on(provider, Platform::Windows),
      Err(PermissionProblem::NotEnforceable {
        option: PermissionOption::NetworkOff,
        reason: Unavailable::CannotBlockNetwork,
      }),
      "{provider:?}"
    );
    let (fitted, resets) = list.fitted_to(provider, Platform::Windows);
    assert_eq!(fitted, list, "{provider:?} keeps the list");
    assert!(!resets.contains(&PermissionOption::NetworkOff));
  }
}
