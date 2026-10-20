//! Task and project memory as the Memory screens list and change it: grouping and filtering, a person's changes
//! under the revision rule and the secret screen, and what the next agent of a task gets. Run with
//! `cargo test -p orchester-workspace --test memory memory`.

mod support;

use std::collections::HashMap;

use orchester_core::{
  id::{SecretId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Importance, Memory, MemoryContent, MemoryFilter, MemoryKey, MemoryKind,
    MemoryScope, MemorySettings, MemoryStatus, MemoryStore, ScopeFilter, SecretScreen,
  },
  resource::StepKey,
  run::Run,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
};
use orchester_workspace::memory::{
  change::{Change, MemoryFailure, change_in, create_in, delete_in},
  draft::{DraftEdit, RecordDraft},
  handoff::{HandoffList, HandoffParts},
  listing::{self, AuthorFilter, Counts, Filters, Group, StatusFilter},
  next::next_agent,
  read::{ProjectMemory, TaskMemory},
};
use parking_lot::Mutex;
use support::{Fixture, at};

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

fn person(scope: MemoryScope, kind: MemoryKind, text: &str, millis: i64) -> Memory {
  let content = MemoryContent::new(kind, text).expect("content");
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_person(scope, screened, at(millis))
}

fn agent(run: &Run, scope: MemoryScope, content: MemoryContent, millis: i64) -> Memory {
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile: orchester_core::id::ResourceId::generate(),
      run: run.id(),
      step: StepKey::new("implement").expect("key"),
      execution: run.current_attempt().expect("attempt").executions()[0].id(),
    },
    confidence: Confidence::High,
    sources: Vec::new(),
  };
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  let after_approval = MemorySettings {
    task_writes: orchester_core::memory::WritePolicy::AfterApproval,
    ..MemorySettings::default()
  };
  Memory::by_agent(scope, write, screened, &after_approval, at(millis))
}

fn keyed(kind: MemoryKind, key: &str, text: &str) -> MemoryContent {
  MemoryContent::new(kind, text)
    .expect("content")
    .with_key(MemoryKey::new(key).expect("key"))
}

fn stored(fixture: &Fixture, id: orchester_core::id::MemoryId) -> Memory {
  fixture.db.memory(id).expect("read").expect("stored")
}

fn texts(listed: &listing::Listed<'_>) -> Vec<String> {
  listed
    .records
    .iter()
    .map(|memory| memory.content().text().to_owned())
    .collect()
}

#[test]
fn memory_listing_groups_waiting_first_then_types_with_pinned_first() {
  let task = MemoryScope::Task(TaskId::generate());
  let fact = person(task, MemoryKind::Fact, "Tests use tempfile.", 1);
  let mut pinned = person(task, MemoryKind::Fact, "Port 8080.", 5);
  pinned.set_pinned(true, at(6)).expect("pin");
  let decision = person(task, MemoryKind::Decision, "Use SQLite.", 2);
  let mut deprecated = person(task, MemoryKind::Warning, "Never VACUUM.", 3);
  deprecated.deprecate(at(4)).expect("deprecate");
  let mut fixture = Fixture::new();
  let id = fixture.task("Webhooks", at(1));
  let run = fixture.started_run(id, at(2));
  let proposal = agent(
    &run,
    task,
    MemoryContent::new(MemoryKind::Todo, "Add a test.").expect("content"),
    7,
  );
  let records = vec![fact, pinned, decision, deprecated, proposal];

  let groups = listing::grouped(&records, &Filters::default(), true);
  let order: Vec<Group> = groups.iter().map(|listed| listed.group).collect();
  assert_eq!(
    order,
    [
      Group::Waiting,
      Group::Kind(MemoryKind::Decision),
      Group::Kind(MemoryKind::Fact)
    ],
    "the deprecated warning is not shown by default, and empty groups are left out"
  );
  assert_eq!(texts(&groups[2]), ["Port 8080.", "Tests use tempfile."]);

  let all = Filters {
    status: StatusFilter::All,
    ..Filters::default()
  };
  assert!(
    listing::grouped(&records, &all, true)
      .iter()
      .any(|listed| listed.group == Group::Kind(MemoryKind::Warning))
  );
  let people = Filters {
    author: AuthorFilter::People,
    ..Filters::default()
  };
  assert!(
    listing::grouped(&records, &people, true)
      .iter()
      .all(|listed| listed.group != Group::Waiting)
  );
  let counts = Counts::of(&records);
  assert_eq!((counts.active, counts.waiting, counts.not_given()), (3, 1, 1));
}

#[test]
fn memory_a_handoff_splits_into_its_summary_and_lists() {
  let parts = HandoffParts::of(
    "Returned the work to Implement.\n\nCompleted:\n- Checked the criteria\n- Ran the tests\n\nThe next agent should \
     know:\n- 429 ignores Retry-After",
  );
  assert_eq!(parts.summary, "Returned the work to Implement.");
  assert_eq!(
    parts.lists,
    [
      (
        HandoffList::Completed,
        vec!["Checked the criteria".to_owned(), "Ran the tests".to_owned()]
      ),
      (
        HandoffList::NextAgentShouldKnow,
        vec!["429 ignores Retry-After".to_owned()]
      ),
    ]
  );
  assert!(HandoffParts::of("Only a summary.").lists.is_empty());
}

#[test]
fn memory_search_finds_parts_of_words_in_text_keys_and_tags_and_counts_other_types() {
  let task = MemoryScope::Task(TaskId::generate());
  let decision = {
    let content = keyed(
      MemoryKind::Decision,
      "webhooks.backoff",
      "Exponential backoff with full jitter.",
    );
    let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
    Memory::by_person(task, screened, at(1))
  };
  let fact = person(task, MemoryKind::Fact, "Retry-After arrives as seconds.", 2);
  let records = vec![decision, fact];
  assert!(listing::found(&records[0], "JITT webhooks"));
  assert!(!listing::found(&records[0], "oauth refresh"));
  let decisions = Filters {
    kind: Some(MemoryKind::Decision),
    search: "retry".to_owned(),
    ..Filters::default()
  };
  assert!(listing::grouped(&records, &decisions, true).is_empty());
  assert_eq!(listing::found_elsewhere(&records, &decisions), 1);
  assert_eq!(listing::of_kind(&records, &decisions), 1);
  assert!(decisions.narrowed());
}

#[test]
fn memory_a_search_says_which_other_status_holds_its_matches() {
  let project = MemoryScope::Project;
  let active = person(project, MemoryKind::Fact, "CI checks run on every pull request.", 1);
  let mut runner = person(project, MemoryKind::Fact, "CI runs on kontur-ci-01.", 2);
  runner.deprecate(at(3)).expect("deprecate");
  let mut old = person(project, MemoryKind::Fact, "CI used kontur-ci-00.", 4);
  old.deprecate(at(5)).expect("deprecate");
  let records = vec![active, runner, old];
  let searched = Filters {
    search: "kontur".to_owned(),
    ..Filters::project()
  };
  assert_eq!(listing::shown(&records, &searched), 0);
  assert_eq!(
    listing::found_in_other_statuses(&records, &searched),
    (2, Some(StatusFilter::Deprecated))
  );
  let deprecated = Filters {
    status: StatusFilter::Deprecated,
    search: "pull request".to_owned(),
    ..Filters::default()
  };
  assert_eq!(
    listing::found_in_other_statuses(&records, &deprecated),
    (1, Some(StatusFilter::Active))
  );
  let every = Filters {
    status: StatusFilter::All,
    search: "nothing like it".to_owned(),
    ..Filters::default()
  };
  assert_eq!(listing::found_in_other_statuses(&records, &every), (0, None));
  assert_eq!(listing::of_status(&records, StatusFilter::Active), 1);
  assert!(listing::lists(&records, &Filters::project(), records[0].id()));
  assert!(!listing::lists(&records, &Filters::project(), records[1].id()));
  assert!(StatusFilter::Active.single() && !StatusFilter::All.single());
}

#[test]
fn memory_approving_with_edits_supersedes_the_record_it_replaces() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let run = fixture.started_run(task, at(2));
  let scope = MemoryScope::Task(task);
  let old = {
    let screened = SecretScreen::new(Vec::new())
      .check(keyed(MemoryKind::Decision, "retry.budget", "A global budget of 5."))
      .expect("no secrets");
    Memory::by_person(scope, screened, at(3))
  };
  let screened = SecretScreen::new(Vec::new())
    .check(keyed(MemoryKind::Decision, "retry.budget", "A budget per endpoint."))
    .expect("no secrets");
  let proposal = Memory::replacing(
    &old,
    AgentWrite {
      provenance: AgentProvenance {
        profile: orchester_core::id::ResourceId::generate(),
        run: run.id(),
        step: StepKey::new("implement").expect("key"),
        execution: run.current_attempt().expect("attempt").executions()[0].id(),
      },
      confidence: Confidence::High,
      sources: Vec::new(),
    },
    screened,
    &MemorySettings {
      task_writes: orchester_core::memory::WritePolicy::AfterApproval,
      ..MemorySettings::default()
    },
    at(4),
  )
  .expect("replaces an active record");
  fixture
    .db
    .add_memories(&[old.clone(), proposal.clone()])
    .expect("store");
  let vault = MemoryVault::default();
  let mut draft = RecordDraft::of(&proposal);
  assert!(draft.edits_of(&proposal).is_empty(), "nothing edited yet");
  draft.text = "A budget per endpoint, at most 10.".to_owned();
  draft.importance = Importance::High;
  assert_eq!(
    draft.edits_of(&proposal),
    [
      DraftEdit::Importance {
        from: proposal.content().importance(),
        to: Importance::High
      },
      DraftEdit::Text
    ]
  );
  change_in(
    fixture.repo.path(),
    &vault,
    (proposal.id(), proposal.revision()),
    Change::Approve(Some(draft)),
  )
  .expect("approved");
  let approved = stored(&fixture, proposal.id());
  assert_eq!(approved.status(), MemoryStatus::Active);
  assert_eq!(approved.content().text(), "A budget per endpoint, at most 10.");
  assert!(approved.edited_at().is_some(), "the person's edit is marked");
  assert_eq!(stored(&fixture, old.id()).status(), MemoryStatus::Superseded);
  let read = TaskMemory::load_in(fixture.repo.path(), task).expect("read");
  assert_eq!(
    listing::earlier_versions(&approved, &read.records)
      .iter()
      .map(|memory| memory.id())
      .collect::<Vec<_>>(),
    [old.id()]
  );
}

#[test]
fn memory_changes_refuse_a_record_changed_since_it_was_read() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let record = person(MemoryScope::Task(task), MemoryKind::Fact, "Port 8080.", 2);
  fixture.db.add_memories(std::slice::from_ref(&record)).expect("store");
  let vault = MemoryVault::default();
  let at_read = (record.id(), record.revision());
  change_in(fixture.repo.path(), &vault, at_read, Change::Pin(true)).expect("pinned");
  let stale = change_in(fixture.repo.path(), &vault, at_read, Change::Deprecate);
  assert!(matches!(stale, Err(MemoryFailure::ChangedMeanwhile)));
  let refused = stale.expect_err("refused");
  assert_eq!(refused.message().key, "memory.errors.changedMeanwhile");
  assert!(refused.stale(), "the screen reads the record again");
  assert!(!MemoryFailure::Draft(orchester_workspace::memory::draft::DraftProblem::BlankText).stale());
  let now = stored(&fixture, record.id());
  assert!(now.is_pinned());
  change_in(
    fixture.repo.path(),
    &vault,
    (now.id(), now.revision()),
    Change::Deprecate,
  )
  .expect("deprecated");
  let deprecated = stored(&fixture, record.id());
  assert_eq!(deprecated.status(), MemoryStatus::Deprecated);
  change_in(
    fixture.repo.path(),
    &vault,
    (deprecated.id(), deprecated.revision()),
    Change::Restore,
  )
  .expect("restored");
  assert_eq!(stored(&fixture, record.id()).status(), MemoryStatus::Active);
}

#[test]
fn memory_a_persons_record_holding_a_secret_value_is_not_saved() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("WEBHOOK_TOKEN").expect("name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    at(1),
  );
  fixture.db.create_secret(&secret).expect("secret");
  let vault = MemoryVault::default();
  vault
    .store(
      &secret.vault_key(fixture.db.project().id()),
      &SecretValue::new("whk-0123456789".to_owned()).expect("value"),
    )
    .expect("stored");
  let mut draft = RecordDraft::blank();
  draft.text = "The token is whk-0123456789.".to_owned();
  let refused = create_in(fixture.repo.path(), &vault, MemoryScope::Task(task), (&draft, false));
  let message = refused.expect_err("refused").message();
  assert_eq!(message.key, "memory.errors.secret");
  assert_eq!(message.args, [("secret", "WEBHOOK_TOKEN".to_owned())]);
  let read = TaskMemory::load_in(fixture.repo.path(), task).expect("read");
  assert!(read.records.is_empty(), "nothing was stored");
}

#[test]
fn memory_new_records_are_active_and_a_key_names_one_record_per_scope() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let vault = MemoryVault::default();
  let mut draft = RecordDraft::blank();
  draft.kind = MemoryKind::Decision;
  draft.key = "db.engine".to_owned();
  draft.text = "SQLite.".to_owned();
  draft = draft.with_tag("Storage").expect("a tag");
  let id = create_in(fixture.repo.path(), &vault, MemoryScope::Task(task), (&draft, true)).expect("created");
  let created = stored(&fixture, id);
  assert_eq!(created.status(), MemoryStatus::Active);
  assert!(created.is_pinned());
  assert_eq!(created.content().tags().joined(), "storage");
  let taken = create_in(fixture.repo.path(), &vault, MemoryScope::Task(task), (&draft, false));
  assert_eq!(taken.expect_err("refused").message().key, "memory.errors.keyTaken");
  create_in(fixture.repo.path(), &vault, MemoryScope::Project, (&draft, false)).expect("another scope");
  let blank = create_in(
    fixture.repo.path(),
    &vault,
    MemoryScope::Project,
    (&RecordDraft::blank(), false),
  );
  assert_eq!(blank.expect_err("refused").message().key, "memory.errors.blankText");
}

#[test]
fn memory_deleting_a_record_deletes_its_history() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let vault = MemoryVault::default();
  let mut draft = RecordDraft::blank();
  draft.text = "Port 8080.".to_owned();
  let id = create_in(fixture.repo.path(), &vault, MemoryScope::Task(task), (&draft, false)).expect("created");
  let record = stored(&fixture, id);
  delete_in(fixture.repo.path(), (id, record.revision())).expect("deleted");
  assert!(fixture.db.memory(id).expect("read").is_none());
  let again = delete_in(fixture.repo.path(), (id, record.revision()));
  assert!(matches!(again, Err(MemoryFailure::Gone)));
}

#[test]
fn memory_project_page_lists_every_proposal_and_the_projects_records() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let run = fixture.started_run(task, at(2));
  let project = person(MemoryScope::Project, MemoryKind::Fact, "Repo layout.", 3);
  let task_proposal = agent(
    &run,
    MemoryScope::Task(task),
    MemoryContent::new(MemoryKind::Warning, "Fake clock.").expect("content"),
    4,
  );
  let project_proposal = support::proposed(&run, "Rate limits.", 5);
  fixture
    .db
    .add_memories(&[project, task_proposal, project_proposal])
    .expect("store");
  let read = ProjectMemory::load_in(fixture.repo.path()).expect("read");
  assert_eq!(read.waiting.len(), 2);
  assert_eq!(read.waiting_by_scope(), (1, 1));
  let counts = read.counts();
  assert_eq!((counts.active, counts.waiting, counts.total()), (1, 2, 3));
  let groups = listing::grouped(&read.records, &Filters::project(), false);
  assert_eq!(groups.len(), 1);
  let written = read.origins.written(&read.waiting[0]).expect("an agent's");
  assert_eq!(written.run, Some(run.number()));
  assert_eq!(written.task.map(|key| key.to_string()), Some("ORC-1".to_owned()));
  assert_eq!(
    written.step.map(|(number, name)| (number, name.to_string())),
    Some((1, "Implement".to_owned()))
  );
  assert_eq!(written.agent.as_deref(), Some("Builder"));
}

#[test]
fn memory_next_agent_is_the_step_that_works_and_none_once_the_run_ended() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Webhooks", at(1));
  let mut run = fixture.started_run(task, at(2));
  let handoff = person(MemoryScope::Task(task), MemoryKind::Handoff, "Planned the store.", 3);
  fixture.db.add_memories(std::slice::from_ref(&handoff)).expect("store");
  let read = fixture.db.memories(&MemoryFilter {
    scope: ScopeFilter::Task(task),
    status: None,
    pinned_only: false,
  });
  assert_eq!(read.expect("read").len(), 1);
  let detail = orchester_core::store::TaskStore::task(&fixture.db, task)
    .expect("read")
    .expect("task");
  let next = next_agent(&fixture.db, &detail, at(4))
    .expect("read")
    .expect("a step works");
  assert_eq!(
    (next.step.number, &*next.step.name, &*next.step.agent),
    (1, "Implement", "Builder")
  );
  assert_eq!(next.given.len(), 1);
  assert_eq!(next.given[0].memory.id(), handoff.id());
  assert!(next.tokens() > 0);
  fixture.succeed(&mut run, at(5));
  assert!(next_agent(&fixture.db, &detail, at(6)).expect("read").is_none());
  let read = TaskMemory::load_in(fixture.repo.path(), task).expect("read");
  assert!(read.next.is_none());
  assert_eq!(listing::latest_handoff(&read.records), Some(handoff.id()));
}
