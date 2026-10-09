//! Records an agent keeps with `save_memory`: active or proposed as the project's setting says for the scope; a key
//! replaces the active record with that key once the note is active; a secret's value refuses the note.

use orchester_core::{
  id::{ResourceId, TaskId},
  memory::{
    Memory, MemoryKey, MemoryKind, MemoryNote, MemoryScope, MemorySettings, MemoryStatus, NoteScope, NoteWrite,
    SecretScreen, WritePolicy,
  },
};

use crate::support::{at, content, provenance, secret_name, secret_value, write};

fn note(scope: NoteScope, key: Option<&str>, text: &str) -> MemoryNote {
  let mut kept = content(MemoryKind::Fact, text);
  if let Some(key) = key {
    kept = kept.with_key(MemoryKey::new(key).expect("a key"));
  }
  MemoryNote::new(scope, kept)
}

#[test]
fn notes_are_kept_as_the_scope_setting_says() {
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let screen = SecretScreen::new(Vec::new());
  let settings = MemorySettings {
    project_writes: WritePolicy::AfterApproval,
    task_writes: WritePolicy::Automatic,
    ..MemorySettings::default()
  };
  let write = NoteWrite {
    task,
    provenance: &agent,
    settings: &settings,
    screen: &screen,
    now: at(5),
  };
  let (changes, kept) = note(NoteScope::Task, None, "The API uses v2.")
    .changes(&write, &[])
    .expect("no secret");
  assert_eq!(
    (kept.scope(), kept.status()),
    (MemoryScope::Task(task), MemoryStatus::Active)
  );
  assert_eq!(changes.added, [kept]);
  assert!(changes.changed.is_empty());
  let (_, kept) = note(NoteScope::Project, None, "Every crate has a tests folder.")
    .changes(&write, &[])
    .expect("no secret");
  assert_eq!(
    (kept.scope(), kept.status()),
    (MemoryScope::Project, MemoryStatus::Proposed)
  );
}

/// A note with the key of an active record replaces it: at once when it is active, only once approved when it is
/// proposed.
#[test]
fn notes_with_a_key_replace_the_active_record() {
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let screen = SecretScreen::new(Vec::new());
  let old_content = content(MemoryKind::Fact, "Port 8080.").with_key(MemoryKey::new("port").expect("a key"));
  let old = Memory::by_agent(
    MemoryScope::Task(task),
    write(ResourceId::generate()),
    SecretScreen::new(Vec::new()).check(old_content).expect("no secret"),
    &MemorySettings::default(),
    at(1),
  );
  assert_eq!(old.status(), MemoryStatus::Active);
  let automatic = MemorySettings::default();
  let write_with = |settings| NoteWrite {
    task,
    provenance: &agent,
    settings,
    screen: &screen,
    now: at(9),
  };
  let (changes, kept) = note(NoteScope::Task, Some("port"), "Port 9090.")
    .changes(&write_with(&automatic), std::slice::from_ref(&old))
    .expect("no secret");
  assert_eq!(kept.status(), MemoryStatus::Active);
  assert_eq!(changes.changed.len(), 1);
  assert_eq!(changes.changed[0].status(), MemoryStatus::Superseded);
  let approval = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    ..MemorySettings::default()
  };
  let (changes, kept) = note(NoteScope::Task, Some("port"), "Port 9090.")
    .changes(&write_with(&approval), std::slice::from_ref(&old))
    .expect("no secret");
  assert_eq!(kept.status(), MemoryStatus::Proposed);
  assert!(
    changes.changed.is_empty(),
    "the old record stays until the note is approved"
  );
  // Another scope's key is no match.
  let (changes, _) = note(NoteScope::Project, Some("port"), "Port 1.")
    .changes(&write_with(&automatic), std::slice::from_ref(&old))
    .expect("no secret");
  assert!(changes.changed.is_empty());
}

#[test]
fn notes_with_a_secret_value_are_refused() {
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let screen = SecretScreen::new(vec![(secret_name("DEPLOY_TOKEN"), secret_value("tok-31337"))]);
  let settings = MemorySettings::default();
  let write = NoteWrite {
    task,
    provenance: &agent,
    settings: &settings,
    screen: &screen,
    now: at(2),
  };
  let refused = note(NoteScope::Task, None, "The token is tok-31337.")
    .changes(&write, &[])
    .expect_err("a secret");
  assert_eq!(refused.secret, secret_name("DEPLOY_TOKEN"));
}

/// A note never takes over a person's record or a pinned one on its own: it waits for approval whatever the setting,
/// and the old record keeps its pin.
#[test]
fn notes_never_replace_a_persons_or_a_pinned_record_alone() {
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let screen = SecretScreen::new(Vec::new());
  let keyed = || {
    SecretScreen::new(Vec::new())
      .check(content(MemoryKind::Fact, "Port 8080.").with_key(MemoryKey::new("port").expect("a key")))
      .expect("no secret")
  };
  let person = Memory::by_person(MemoryScope::Task(task), keyed(), at(1));
  let mut pinned = Memory::by_agent(
    MemoryScope::Task(task),
    write(ResourceId::generate()),
    keyed(),
    &MemorySettings::default(),
    at(1),
  );
  pinned.set_pinned(true, at(2)).expect("pinned");
  let settings = MemorySettings::default();
  let write = NoteWrite {
    task,
    provenance: &agent,
    settings: &settings,
    screen: &screen,
    now: at(9),
  };
  for old in [person, pinned] {
    let (changes, kept) = note(NoteScope::Task, Some("port"), "Port 9090.")
      .changes(&write, std::slice::from_ref(&old))
      .expect("no secret");
    assert_eq!(kept.status(), MemoryStatus::Proposed);
    assert!(!kept.is_pinned());
    assert!(changes.changed.is_empty(), "the old record stays as it is");
  }
}
