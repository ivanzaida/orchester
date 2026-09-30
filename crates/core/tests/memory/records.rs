//! Records: who writes them and how they are kept, their transitions, their content rules and the
//! secret values kept out of them. Run with `cargo test -p orchester-core --test memory records`.

use orchester_core::{
  id::{ProjectId, ResourceId, SecretId, TaskId},
  memory::{
    ContainsSecret, Importance, Memory, MemoryContent, MemoryContentError, MemoryError, MemoryKey, MemoryKind,
    MemoryLimits, MemoryScope, MemorySettings, MemoryStatus, SecretScreen, Tags, WritePolicy,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, VaultError},
};

use super::support::{Keychain, at, content, provenance, screened, secret_name, secret_value};

#[test]
fn records_follow_the_write_policy_of_their_scope() {
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let write = |scope, settings: &MemorySettings| {
    Memory::by_agent(
      scope,
      agent.clone(),
      screened(MemoryKind::Fact, "Tests use tempfile."),
      vec![7],
      settings,
      at(1),
    )
    .status()
  };
  let defaults = MemorySettings::default();
  assert_eq!(defaults.limits, MemoryLimits::DEFAULT);
  assert_eq!(defaults.limits.records.get(), 8);
  assert_eq!(defaults.limits.tokens.get(), 3_000);
  assert_eq!(write(MemoryScope::Task(task), &defaults), MemoryStatus::Active);
  assert_eq!(write(MemoryScope::Project, &defaults), MemoryStatus::Proposed);
  let reversed = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    project_writes: WritePolicy::Automatic,
    ..defaults
  };
  assert_eq!(write(MemoryScope::Task(task), &reversed), MemoryStatus::Proposed);
  assert_eq!(write(MemoryScope::Project, &reversed), MemoryStatus::Active);
  let person = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Decision, "Use SQLite."),
    at(1),
  );
  assert_eq!(person.status(), MemoryStatus::Active);
}

#[test]
fn records_a_proposal_is_approved_or_rejected_once() {
  let mut proposal = Memory::by_agent(
    MemoryScope::Project,
    provenance(ResourceId::generate()),
    screened(MemoryKind::Warning, "The CI cache is flaky."),
    Vec::new(),
    &MemorySettings::default(),
    at(1),
  );
  let mut rejected = proposal.clone();
  proposal.approve(at(2)).expect("approve");
  assert_eq!(proposal.status(), MemoryStatus::Active);
  assert_eq!(proposal.updated_at(), at(2));
  assert!(matches!(
    proposal.approve(at(3)),
    Err(MemoryError::NotProposed {
      status: MemoryStatus::Active,
      ..
    })
  ));
  rejected.reject(at(2)).expect("reject");
  assert_eq!(rejected.status(), MemoryStatus::Deprecated);
  assert!(rejected.deprecate(at(3)).is_err());
}

#[test]
fn records_supersede_deprecate_edit_and_pin() {
  let mut first = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Decision, "Use Postgres."),
    at(1),
  );
  first.set_pinned(true, at(2)).expect("pin");
  let replacement = first
    .supersede(screened(MemoryKind::Decision, "Use SQLite."), at(3))
    .expect("supersede");
  assert_eq!(first.status(), MemoryStatus::Superseded);
  assert!(!first.is_pinned());
  assert_eq!(replacement.supersedes(), Some(first.id()));
  assert_eq!(replacement.scope(), MemoryScope::Project);
  assert!(replacement.is_pinned(), "the replacement keeps the pin");
  assert!(matches!(
    first.edit(screened(MemoryKind::Decision, "Use MySQL."), at(4)),
    Err(MemoryError::Closed { .. })
  ));
  assert!(first.supersede(screened(MemoryKind::Decision, "x"), at(4)).is_err());

  let mut current = replacement;
  current
    .edit(screened(MemoryKind::Decision, "Use SQLite with FTS5."), at(5))
    .expect("edit");
  assert_eq!(current.content().text(), "Use SQLite with FTS5.");
  current.deprecate(at(6)).expect("deprecate");
  assert_eq!(current.status(), MemoryStatus::Deprecated);
  assert!(!current.is_pinned());
  assert!(current.set_pinned(true, at(7)).is_err());
}

#[test]
fn records_content_is_validated() {
  assert_eq!(
    MemoryContent::new(MemoryKind::Fact, "  \n"),
    Err(MemoryContentError::BlankText)
  );
  assert_eq!(MemoryKey::new("a\nb"), Err(MemoryContentError::MultilineKey));
  assert!(MemoryKey::new(&"k".repeat(121)).is_err());
  assert_eq!(Tags::new(["two words"]), Err(MemoryContentError::InvalidTag));
  let tags = Tags::new(["Auth", "auth", "db"]).expect("tags");
  assert_eq!(tags.joined(), "auth db");
  assert_eq!(Importance::new(0), Err(MemoryContentError::Importance(0)));
  assert!(Importance::new(5).is_ok());
  let content = content(MemoryKind::Todo, "  Rotate the key.  ");
  assert_eq!(content.text(), "Rotate the key.");
}

#[test]
fn records_holding_a_secret_value_are_refused_without_echoing_it() {
  let value = "fake-token-Zq81x";
  let screen = SecretScreen::new(vec![(secret_name("DEPLOY_TOKEN"), secret_value(&format!("{value}\n")))]);
  let leaked = content(MemoryKind::Fact, &format!("Deploy with {value} from CI."));
  let refused = screen.check(leaked).expect_err("the value is refused");
  assert_eq!(
    refused,
    ContainsSecret {
      secret: secret_name("DEPLOY_TOKEN")
    }
  );
  let message = refused.to_string();
  assert!(message.contains("DEPLOY_TOKEN"), "{message}");
  assert!(!message.contains(value), "{message}");
  assert!(!format!("{refused:?}").contains(value));
  assert!(!format!("{screen:?}").contains(value));

  let in_key = content(MemoryKind::Fact, "Deploys use a token.").with_key(MemoryKey::new(value).expect("key"));
  assert!(screen.check(in_key).is_err());
  let in_tags = content(MemoryKind::Fact, "Deploys use a token.").with_tags(Tags::new([value]).expect("tags"));
  assert!(screen.check(in_tags).is_err());
  assert!(screen.check(content(MemoryKind::Fact, "Deploys use a token.")).is_ok());
}

#[test]
fn records_screen_reads_the_values_of_active_secrets() {
  let project = ProjectId::generate();
  let secret = |name: &str| {
    Secret::new(
      SecretId::generate(),
      NewSecret {
        name: secret_name(name),
        kind: SecretKind::ApiKey,
        environment: Environment::Development,
      },
      at(1),
    )
  };
  let (stored, missing) = (secret("API_KEY"), secret("OLD_KEY"));
  let keychain = Keychain::with(project, &[(&stored, "fake-key-77aa")]);
  let screen = SecretScreen::load(project, &[stored.clone(), missing], &keychain).expect("screen");
  assert!(screen.check(content(MemoryKind::Fact, "Key fake-key-77aa.")).is_err());
  assert!(screen.check(content(MemoryKind::Fact, "No key here.")).is_ok());

  let locked = Keychain {
    failure: Some(VaultError::AccessDenied("locked".to_owned())),
    ..Keychain::default()
  };
  let unavailable = SecretScreen::load(project, &[stored], &locked).expect_err("cannot check");
  assert_eq!(unavailable.secret, secret_name("API_KEY"));
}
