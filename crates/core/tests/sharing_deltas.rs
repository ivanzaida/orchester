use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::{ClaimId, ModuleId, TagId, TaskId, UserId},
  name::Name,
  run::TaskPhase,
  sharing::{
    CachedClaim, CachedRunStatus, CachedTask, ClaimKind, DeltaOutcome, RunProgress, SharedPerson, TagRemap, TaskChange,
    TaskDelta, TaskDeltaKind, TaskRevision, remap_tags,
  },
  task::{
    TaskLimits, TaskStatus,
    organization::{BlockingTask, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
  tokens::TokenBudget,
};

fn at(ms: i64) -> Timestamp {
  Timestamp::from_unix_millis(ms)
}

fn revision(value: u64) -> TaskRevision {
  TaskRevision::try_from(value).expect("a revision above zero")
}

fn person(name: &str) -> SharedPerson {
  SharedPerson {
    id: UserId::generate(),
    name: name.to_owned(),
  }
}

fn number(value: u32) -> NonZeroU32 {
  NonZeroU32::new(value).expect("a number above zero")
}

fn task(tags: Vec<TagId>) -> CachedTask {
  let maria = person("Maria Lind");
  CachedTask {
    id: TaskId::generate(),
    number: number(12),
    revision: revision(4),
    title: Name::new("Import invoices").expect("title"),
    brief: "Read the CSV".to_owned(),
    criteria: vec!["Rows imported".to_owned()],
    pipeline: Some("default".to_owned()),
    limits: TaskLimits::NONE,
    status: TaskStatus::Draft,
    organization: TaskOrganization::new(WorkflowStatus::Todo, Priority::Medium, None, Vec::new(), tags)
      .expect("organization"),
    module: None,
    former_key: None,
    created_by: maria.name.clone(),
    created_at: at(100),
    updated_by: maria.name.clone(),
    updated_at: at(200),
    claim: None,
    run: Some(CachedRunStatus {
      holder: maria,
      number: number(2),
      phase: TaskPhase::Running,
      step: Some("Implement".to_owned()),
      started_at: at(300),
      phase_since: at(300),
      updated_at: at(400),
    }),
  }
}

fn delta(change: TaskDeltaKind) -> TaskDelta {
  TaskDelta {
    by: person("Alice Moreau"),
    at: at(1_000),
    change,
  }
}

fn changed(change: TaskChange) -> TaskDelta {
  delta(TaskDeltaKind::Changed(change))
}

fn applied(outcome: DeltaOutcome) -> CachedTask {
  match outcome {
    DeltaOutcome::Applied(task) => *task,
    other => panic!("expected the delta applied, got {other:?}"),
  }
}

/// Applies `change`, made on the cached revision, and checks that only the revision, who changed it and the fields
/// `expected` sets moved.
fn assert_applies(change: TaskChange, expected: impl FnOnce(&mut CachedTask)) {
  let cached = task(Vec::new());
  let after = applied(cached.apply(&changed(change)));
  let mut want = cached;
  want.revision = revision(5);
  want.updated_by = "Alice Moreau".to_owned();
  want.updated_at = at(1_000);
  expected(&mut want);
  assert_eq!(after, want);
}

fn next() -> TaskChange {
  TaskChange::new(revision(4), revision(5))
}

#[test]
fn task_changed_applies_each_content_field() {
  let title = Name::new("Import receipts").expect("title");
  assert_applies(
    TaskChange {
      title: Some(title.clone()),
      ..next()
    },
    |task| task.title = title,
  );
  assert_applies(
    TaskChange {
      brief: Some("Read the XLSX".to_owned()),
      ..next()
    },
    |task| task.brief = "Read the XLSX".to_owned(),
  );
  let criteria = vec!["Rows imported".to_owned(), "Errors listed".to_owned()];
  assert_applies(
    TaskChange {
      criteria: Some(criteria.clone()),
      ..next()
    },
    |task| task.criteria = criteria,
  );
  assert_applies(
    TaskChange {
      pipeline: Some(None),
      ..next()
    },
    |task| task.pipeline = None,
  );
  let limits = TaskLimits::new(
    Some(TokenBudget::new(50_000).expect("budget")),
    Some(Duration::from_secs(600)),
  )
  .expect("limits");
  assert_applies(
    TaskChange {
      limits: Some(limits),
      ..next()
    },
    |task| task.limits = limits,
  );
  assert_applies(
    TaskChange {
      status: Some(TaskStatus::Queued),
      ..next()
    },
    |task| task.status = TaskStatus::Queued,
  );
  let module = ModuleId::generate();
  assert_applies(
    TaskChange {
      module: Some(Some(module)),
      ..next()
    },
    |task| task.module = Some(module),
  );
}

#[test]
fn task_changed_applies_each_organization_field() {
  assert_applies(
    TaskChange {
      priority: Some(Priority::Urgent),
      ..next()
    },
    |task| task.organization = task.organization.clone().with_priority(Priority::Urgent),
  );
  assert_applies(
    TaskChange {
      workflow: Some(WorkflowStatus::InReview),
      ..next()
    },
    |task| task.organization = task.organization.clone().with_status(WorkflowStatus::InReview),
  );
  let blocker = TaskId::generate();
  let links = vec![BlockingTask {
    task: blocker,
    gate: DependencyGate::Merge,
  }];
  assert_applies(
    TaskChange {
      blocked_by: Some(links.clone()),
      blocked_reason: Some(Some("Waits for the parser".to_owned())),
      ..next()
    },
    |task| {
      task.organization = TaskOrganization::new(
        WorkflowStatus::Todo,
        Priority::Medium,
        Some("Waits for the parser".to_owned()),
        links,
        Vec::new(),
      )
      .expect("organization");
    },
  );
}

#[test]
fn task_changed_adds_and_removes_tags() {
  let (kept, removed, added) = (TagId::generate(), TagId::generate(), TagId::generate());
  let cached = task(vec![kept, removed]);
  let after = applied(cached.apply(&changed(TaskChange {
    tags_added: vec![added],
    tags_removed: vec![removed],
    ..next()
  })));
  let mut want = vec![kept, added];
  want.sort();
  assert_eq!(after.organization.tags(), want.as_slice());
  assert_eq!(after.revision, revision(5));
}

#[test]
fn task_changed_on_another_revision_is_stale() {
  let cached = task(Vec::new());
  let missed = TaskChange {
    title: Some(Name::new("Import receipts").expect("title")),
    ..TaskChange::new(revision(5), revision(6))
  };
  assert_eq!(cached.apply(&changed(missed)), DeltaOutcome::Stale);
}

#[test]
fn own_or_older_task_changed_is_already_had() {
  let cached = task(Vec::new());
  // The person's own change: the cache took the server's answer, at the delta's revision.
  let own = TaskChange {
    brief: Some("Read the CSV".to_owned()),
    ..TaskChange::new(revision(3), revision(4))
  };
  assert_eq!(cached.apply(&changed(own)), DeltaOutcome::AlreadyHave);
  let older = TaskChange {
    brief: Some("Read the TSV".to_owned()),
    ..TaskChange::new(revision(2), revision(3))
  };
  assert_eq!(cached.apply(&changed(older)), DeltaOutcome::AlreadyHave);
}

#[test]
fn task_claim_replaces_only_the_claim() {
  let cached = task(Vec::new());
  let claim = CachedClaim {
    id: ClaimId::generate(),
    holder: person("Alice Moreau"),
    kind: ClaimKind::Manual,
    since: at(900),
    heartbeat_at: at(900),
    stale: false,
    taken_over_from: None,
  };
  let taken = applied(cached.apply(&delta(TaskDeltaKind::Claim(Some(claim.clone())))));
  assert_eq!(
    taken,
    CachedTask {
      claim: Some(claim.clone()),
      ..cached.clone()
    }
  );
  assert_eq!(
    taken.apply(&delta(TaskDeltaKind::Claim(Some(claim.clone())))),
    DeltaOutcome::AlreadyHave
  );
  let stale = CachedClaim { stale: true, ..claim };
  let marked = applied(taken.apply(&delta(TaskDeltaKind::Claim(Some(stale.clone())))));
  assert_eq!(marked.claim, Some(stale));
  let released = applied(marked.apply(&delta(TaskDeltaKind::Claim(None))));
  assert_eq!(released, cached);
}

fn progress(run: u32, updated_at: i64) -> RunProgress {
  RunProgress {
    number: number(run),
    phase: TaskPhase::Waiting,
    step: Some("Review".to_owned()),
    phase_since: at(updated_at),
    updated_at: at(updated_at),
  }
}

#[test]
fn task_run_updates_only_the_progress() {
  let cached = task(Vec::new());
  let after = applied(cached.apply(&delta(TaskDeltaKind::Run(progress(2, 500)))));
  let before = cached.run.clone().expect("a cached run");
  assert_eq!(
    after,
    CachedTask {
      run: Some(CachedRunStatus {
        phase: TaskPhase::Waiting,
        step: Some("Review".to_owned()),
        phase_since: at(500),
        updated_at: at(500),
        ..before
      }),
      ..cached
    }
  );
}

#[test]
fn task_run_not_newer_is_already_had_and_an_unknown_run_is_stale() {
  let cached = task(Vec::new());
  assert_eq!(
    cached.apply(&delta(TaskDeltaKind::Run(progress(2, 400)))),
    DeltaOutcome::AlreadyHave
  );
  assert_eq!(
    cached.apply(&delta(TaskDeltaKind::Run(progress(1, 900)))),
    DeltaOutcome::AlreadyHave
  );
  assert_eq!(
    cached.apply(&delta(TaskDeltaKind::Run(progress(3, 900)))),
    DeltaOutcome::Stale
  );
  let never_ran = CachedTask { run: None, ..cached };
  assert_eq!(
    never_ran.apply(&delta(TaskDeltaKind::Run(progress(1, 900)))),
    DeltaOutcome::Stale
  );
}

#[test]
fn tag_deleted_removes_it_from_the_cached_tasks() {
  let (gone, other) = (TagId::generate(), TagId::generate());
  let tagged = task(vec![gone, other]);
  let untouched = task(vec![other]);
  let remapped = remap_tags([&tagged, &untouched], TagRemap::Deleted(gone));
  assert_eq!(remapped.len(), 1);
  assert_eq!(remapped[0].id, tagged.id);
  assert_eq!(remapped[0].organization.tags(), [other].as_slice());
  assert_eq!(remapped[0].revision, tagged.revision);
}

#[test]
fn tag_merged_moves_the_cached_tasks_to_the_tag_it_went_into() {
  let (from, into, other) = (TagId::generate(), TagId::generate(), TagId::generate());
  let moved = task(vec![from, other]);
  let had_both = task(vec![from, into]);
  let untouched = task(vec![into]);
  let remapped = remap_tags([&moved, &had_both, &untouched], TagRemap::Merged { from, into });
  let mut want = vec![into, other];
  want.sort();
  assert_eq!(remapped.len(), 2);
  assert_eq!(remapped[0].organization.tags(), want.as_slice());
  assert_eq!(remapped[1].id, had_both.id);
  assert_eq!(remapped[1].organization.tags(), [into].as_slice());
}
