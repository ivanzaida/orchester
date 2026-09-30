//! Records: who writes them and how they are kept, their transitions, their content rules and the
//! secret values kept out of them. Run with `cargo test -p orchester-core --test memory records`.

use orchester_core::{
  id::{ProjectId, ResourceId, SecretId, TaskId},
  memory::{
    Confidence, ContainsSecret, Importance, Memory, MemoryContent, MemoryContentError, MemoryError, MemoryKey,
    MemoryKind, MemoryLimits, MemoryLimitsError, MemoryScope, MemorySettings, MemoryStatus, SecretScreen, Tags,
    WritePolicy,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, VaultError},
};

use super::support::{Keychain, at, content, screened, secret_name, secret_value, write};

#[test]
fn records_follow_the_write_policy_of_their_scope() {
  let task = TaskId::generate();
  let profile = ResourceId::generate();
  let written = |scope, settings: &MemorySettings| {
    Memory::by_agent(
      scope,
      write(profile),
      screened(MemoryKind::Fact, "Tests use tempfile."),
      settings,
      at(1),
    )
  };
  let defaults = MemorySettings::default();
  assert_eq!(defaults.limits, MemoryLimits::DEFAULT);
  assert_eq!((defaults.limits.records(), defaults.limits.tokens()), (8, 3_000));
  assert_eq!(
    written(MemoryScope::Task(task), &defaults).status(),
    MemoryStatus::Active
  );
  assert_eq!(
    written(MemoryScope::Project, &defaults).status(),
    MemoryStatus::Proposed
  );
  let reversed = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    project_writes: WritePolicy::Automatic,
    ..defaults
  };
  assert_eq!(
    written(MemoryScope::Task(task), &reversed).status(),
    MemoryStatus::Proposed
  );
  assert_eq!(written(MemoryScope::Project, &reversed).status(), MemoryStatus::Active);
  assert_eq!(
    written(MemoryScope::Project, &reversed).confidence(),
    Confidence::Medium
  );
  let person = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Decision, "Use SQLite."),
    at(1),
  );
  assert_eq!(person.status(), MemoryStatus::Active);
  assert_eq!(person.confidence(), Confidence::High);
}

#[test]
fn records_limits_are_validated() {
  assert!(MemoryLimits::new(1, 500).is_ok());
  assert!(MemoryLimits::new(50, 20_000).is_ok());
  assert_eq!(MemoryLimits::new(0, 3_000), Err(MemoryLimitsError::Records(0)));
  assert_eq!(MemoryLimits::new(51, 3_000), Err(MemoryLimitsError::Records(51)));
  assert_eq!(MemoryLimits::new(8, 499), Err(MemoryLimitsError::Tokens(499)));
  assert_eq!(MemoryLimits::new(8, 20_001), Err(MemoryLimitsError::Tokens(20_001)));
}

#[test]
fn records_a_proposal_is_approved_or_rejected_once() {
  let mut proposal = Memory::by_agent(
    MemoryScope::Project,
    write(ResourceId::generate()),
    screened(MemoryKind::Warning, "The CI cache is flaky."),
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
fn records_an_agent_supersedes_an_active_record_once_its_replacement_is_active() {
  let mut first = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Decision, "Use Postgres."),
    at(1),
  );
  first.set_pinned(true, at(2)).expect("pin");
  let approval = MemorySettings::default();
  let mut proposed = Memory::replacing(
    &first,
    write(ResourceId::generate()),
    screened(MemoryKind::Decision, "Use SQLite."),
    &approval,
    at(3),
  )
  .expect("a replacement");
  assert_eq!(
    proposed.status(),
    MemoryStatus::Proposed,
    "project memory is saved after approval"
  );
  assert_eq!(proposed.supersedes(), Some(first.id()));
  assert!(matches!(
    first.supersede(&mut proposed, at(4)),
    Err(MemoryError::NotItsReplacement { .. })
  ));
  proposed.approve(at(5)).expect("approve");
  first.supersede(&mut proposed, at(5)).expect("supersede");
  assert_eq!(first.status(), MemoryStatus::Superseded);
  assert!(!first.is_pinned());
  assert!(proposed.is_pinned(), "the replacement takes over the pin");
  assert!(matches!(
    first.edit(screened(MemoryKind::Decision, "Use MySQL."), at(6)),
    Err(MemoryError::Closed { .. })
  ));
  assert!(
    Memory::replacing(
      &first,
      write(ResourceId::generate()),
      screened(MemoryKind::Decision, "x"),
      &approval,
      at(6)
    )
    .is_err()
  );
}

#[test]
fn records_a_person_edits_in_place_deprecates_and_restores() {
  let mut record = Memory::by_agent(
    MemoryScope::Task(TaskId::generate()),
    write(ResourceId::generate()),
    screened(MemoryKind::Decision, "Use SQLite."),
    &MemorySettings::default(),
    at(1),
  );
  let author = record.author().clone();
  assert_eq!(record.edited_at(), None);
  record
    .edit(screened(MemoryKind::Decision, "Use SQLite with FTS5."), at(5))
    .expect("edit");
  assert_eq!(record.content().text(), "Use SQLite with FTS5.");
  assert_eq!(record.author(), &author, "the author stays");
  assert_eq!(record.edited_at(), Some(at(5)));
  assert_eq!(record.supersedes(), None);
  record.set_pinned(true, at(6)).expect("pin");
  record.deprecate(at(7)).expect("deprecate");
  assert_eq!(record.status(), MemoryStatus::Deprecated);
  assert!(!record.is_pinned());
  assert!(record.set_pinned(true, at(8)).is_err());
  record.restore(at(9)).expect("restore");
  assert_eq!(record.status(), MemoryStatus::Active);
  assert!(matches!(record.restore(at(10)), Err(MemoryError::NotDeprecated { .. })));
}

#[test]
fn records_content_is_validated() {
  assert_eq!(
    MemoryContent::new(
      MemoryKind::Fact,
      "  
"
    ),
    Err(MemoryContentError::BlankText)
  );
  assert_eq!(
    MemoryKey::new(
      "a
b"
    ),
    Err(MemoryContentError::MultilineKey)
  );
  assert!(MemoryKey::new(&"k".repeat(121)).is_err());
  assert_eq!(Tags::new(["two words"]), Err(MemoryContentError::InvalidTag));
  let tags = Tags::new(["Auth", "auth", "db"]).expect("tags");
  assert_eq!(tags.joined(), "auth db");
  let content = content(MemoryKind::Todo, "  Rotate the key.  ");
  assert_eq!(content.text(), "Rotate the key.");
  assert_eq!(content.importance(), Importance::Normal);
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
