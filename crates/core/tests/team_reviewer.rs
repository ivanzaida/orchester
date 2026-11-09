use orchester_core::{
  connection::{AgentProvider, DecisionServerUrl},
  resource::{
    Draft, ModelId, ResourceBody, ResourceKind, ReviewerDestinations, ReviewerHost, TeamReviewer, VersionNumber,
    publish,
  },
  run::review::Probability,
  sharing::{VersionWidening, widens},
  time::Timestamp,
};
use serde_json::json;

fn probability(value: f64) -> Probability {
  Probability::new(value).expect("valid probability")
}

fn decision(hosts: &[&str], threshold: f64) -> TeamReviewer {
  decision_named("decision-1", hosts, threshold)
}

fn decision_named(model: &str, hosts: &[&str], threshold: f64) -> TeamReviewer {
  TeamReviewer::Decision {
    model: ModelId::new(model).expect("model"),
    lowest_threshold: probability(threshold),
    destinations: if hosts.is_empty() {
      ReviewerDestinations::MembersComputers
    } else {
      ReviewerDestinations::Hosts(
        hosts
          .iter()
          .map(|host| ReviewerHost::new(host).expect("host"))
          .collect(),
      )
    },
  }
}

fn language() -> TeamReviewer {
  TeamReviewer::Language {
    cli: AgentProvider::CodexCli,
    model: ModelId::new("language-1").expect("model"),
    effort: Some("high".into()),
  }
}

#[test]
fn team_reviewer_is_a_versioned_shared_resource_that_names_no_connection() {
  assert_eq!(TeamReviewer::KIND, ResourceKind::TeamReviewer);
  for reviewer in [language(), decision(&["review.example.org"], 0.8)] {
    let json = serde_json::to_value(&reviewer).expect("serialize");
    assert!(json.get("connection").is_none());
    assert_eq!(
      serde_json::from_value::<TeamReviewer>(json).expect("deserialize"),
      reviewer
    );
    let now = Timestamp::from_unix_millis(100);
    let first = publish(Draft::new(reviewer.clone(), None, now), None, now).expect("publish");
    let mut draft = first.to_draft(now);
    draft.edit(decision(&[], 0.9), now);
    let second = publish(draft, Some(first.number()), now).expect("publish second");
    assert_eq!(first.number(), VersionNumber::FIRST);
    assert_eq!(first.body(), &reviewer);
    assert_eq!(second.number().get(), 2);
  }
}

#[test]
fn team_reviewer_wire_form_names_the_cli_kind_of_a_language_reviewer_only() {
  assert_eq!(
    serde_json::to_value(language()).expect("serialize"),
    json!({"kind": "language", "cli": "codex_cli", "model": "language-1", "effort": "high"})
  );
  assert_eq!(
    serde_json::to_value(decision(&["review.example.org"], 0.8)).expect("serialize"),
    json!({
      "kind": "decision", "model": "decision-1", "lowest_threshold": 0.8,
      "destinations": {"kind": "hosts", "hosts": ["review.example.org"]}
    })
  );
  assert_eq!(language().cli(), Some(AgentProvider::CodexCli));
  assert_eq!(decision(&[], 0.8).cli(), None);
}

#[test]
fn team_reviewer_bodies_that_name_a_connection_are_not_read() {
  let old_language = json!({
    "kind": "language", "connection": {"kind": "connection", "name": "Team language", "role": "codex_cli"},
    "model": "language-1", "effort": "high"
  });
  let old_decision = json!({
    "kind": "decision", "connection": {"kind": "decision_connection", "name": "Team decisions", "role": "decision"},
    "model": "decision-1", "lowest_threshold": 0.8, "destinations": {"kind": "members_computers"}
  });
  for old in [old_language, old_decision] {
    assert!(serde_json::from_value::<TeamReviewer>(old).is_err());
  }
  // A new body with a stray connection field is refused too, and a language reviewer must name its CLI kind.
  let mut stray = serde_json::to_value(language()).expect("serialize");
  stray["connection"] = json!({"name": "x"});
  assert!(serde_json::from_value::<TeamReviewer>(stray).is_err());
  assert!(serde_json::from_value::<TeamReviewer>(json!({"kind": "language", "model": "m", "effort": null})).is_err());
}

#[test]
fn team_reviewer_widens_ignore_the_cli_kind_a_language_reviewer_names() {
  let codex = language();
  let claude = TeamReviewer::Language {
    cli: AgentProvider::ClaudeCodeCli,
    model: ModelId::new("language-1").expect("model"),
    effort: None,
  };
  assert!(!claude.widens_over(Some(&codex)));
  assert!(widens(Some(&codex), &claude).is_empty());
}

#[test]
fn team_reviewer_widens_only_when_hosts_are_added_or_threshold_is_lowered() {
  let local = decision(&[], 0.8);
  let hosted = decision(&["review.example.org"], 0.8);
  assert!(!local.widens_over(None));
  assert!(hosted.widens_over(None));
  assert_eq!(widens(Some(&local), &hosted), [VersionWidening::Address]);
  assert!(!hosted.widens_over(Some(&hosted)));
  assert!(!local.widens_over(Some(&hosted)));
  assert!(decision(&["review.example.org", "other.example.org"], 0.8).widens_over(Some(&hosted)));
  assert!(!decision(&["REVIEW.EXAMPLE.ORG"], 0.9).widens_over(Some(&hosted)));
  assert_eq!(widens(Some(&local), &decision(&[], 0.7)), [VersionWidening::Access]);
  assert_eq!(
    widens(Some(&local), &decision(&["review.example.org"], 0.7)),
    [VersionWidening::Address, VersionWidening::Access],
  );
  assert!(!language().widens_over(None));
  assert!(!language().widens_over(Some(&hosted)));
  assert!(hosted.widens_over(Some(&language())));
  assert!(!decision(&["localhost", "127.0.0.1", "::1"], 0.8).widens_over(Some(&local)));
}

#[test]
fn team_reviewer_effective_threshold_cannot_be_lower_than_the_team() {
  let team = decision(&[], 0.8);
  for (member, expected) in [(None, 0.8), (Some(0.6), 0.8), (Some(0.8), 0.8), (Some(0.95), 0.95)] {
    assert_eq!(
      team.effective_threshold(member.map(probability)),
      Some(probability(expected))
    );
  }
  assert_eq!(language().effective_threshold(Some(probability(0.9))), None);
}

#[test]
fn team_reviewer_binding_server_must_be_local_or_an_exact_listed_host() {
  let local = decision(&[], 0.8);
  let hosted = decision(&["review.example.org", "2001:db8::1"], 0.8);
  for url in [
    "http://localhost:8000",
    "http://127.0.0.1",
    "http://[::1]:9000",
    "http://LOCALHOST",
  ] {
    let server = DecisionServerUrl::new(url).expect("server");
    assert!(local.allows_server(&server));
    assert!(hosted.allows_server(&server));
  }
  for (url, allowed) in [
    ("https://REVIEW.EXAMPLE.ORG:9000/api", true),
    ("http://[2001:0db8:0:0::1]:8000", true),
    ("https://sub.review.example.org", false),
    ("https://review.example.org.evil.test", false),
    ("https://other.example.org", false),
    ("http://0.0.0.0", false),
    ("http://192.168.1.2", false),
    ("http://127.0.0.2", false),
  ] {
    let server = DecisionServerUrl::new(url).expect("server");
    assert_eq!(hosted.allows_server(&server), allowed, "{url}");
    assert!(!local.allows_server(&server), "{url}");
    assert!(!language().allows_server(&server));
  }
}

#[test]
fn team_reviewer_rejects_invalid_hosts_and_stored_values() {
  for host in [
    "",
    "*.example.org",
    "https://example.org",
    "example.org/path",
    "example.org:80",
    "user@example.org",
  ] {
    assert!(ReviewerHost::new(host).is_err(), "{host}");
  }
  assert_eq!(
    ReviewerHost::new("[2001:0db8::1]").expect("IPv6").as_str(),
    "2001:db8::1"
  );
  let mut json = serde_json::to_value(decision(&[], 0.8)).expect("serialize");
  json["lowest_threshold"] = serde_json::json!(1.1);
  assert!(serde_json::from_value::<TeamReviewer>(json).is_err());
  let mut json = serde_json::to_value(language()).expect("serialize");
  json["cli"] = serde_json::json!("github");
  assert!(serde_json::from_value::<TeamReviewer>(json).is_err());
}
