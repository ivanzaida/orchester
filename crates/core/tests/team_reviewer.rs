use orchester_core::{
  connection::{AgentProvider, DecisionServerUrl},
  name::Name,
  resource::{
    DecisionReviewerReference, DecisionReviewerRole, Draft, LanguageReviewerReference, ModelId, ResourceBody,
    ResourceKind, ReviewerDestinations, ReviewerHost, TeamReviewer, VersionNumber, publish,
  },
  run::review::Probability,
  sharing::{VersionWidening, widens},
  time::Timestamp,
};

fn probability(value: f64) -> Probability {
  Probability::new(value).expect("valid probability")
}

fn decision(hosts: &[&str], threshold: f64) -> TeamReviewer {
  TeamReviewer::Decision {
    connection: DecisionReviewerReference::DecisionConnection {
      name: Name::new("Team decisions").expect("name"),
      role: DecisionReviewerRole::Decision,
    },
    model: ModelId::new("decision-1").expect("model"),
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
    connection: LanguageReviewerReference::Connection {
      name: Name::new("Team language").expect("name"),
      role: AgentProvider::CodexCli,
    },
    model: ModelId::new("language-1").expect("model"),
    effort: Some("high".into()),
  }
}

#[test]
fn team_reviewer_is_a_versioned_shared_resource_with_connection_references() {
  assert_eq!(TeamReviewer::KIND, ResourceKind::TeamReviewer);
  for reviewer in [language(), decision(&["review.example.org"], 0.8)] {
    let json = serde_json::to_value(&reviewer).expect("serialize");
    assert!(json["connection"]["name"].is_string());
    assert!(json["connection"]["kind"].is_string());
    assert!(json["connection"]["role"].is_string());
    assert!(json["connection"].get("id").is_none());
    assert!(json["connection"].get("base_url").is_none());
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
  json["connection"]["role"] = serde_json::json!("github");
  assert!(serde_json::from_value::<TeamReviewer>(json).is_err());
}
