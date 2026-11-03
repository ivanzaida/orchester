use orchester_core::{
  id::{ModuleId, OperationId, ProjectId, ResourceId, TagId, TaskId, WorkId},
  sharing::{ExpectedRevision, ExpectedRevisionError, ExpectedRevisions, RevisionTarget},
};
use serde_json::json;
use uuid::Uuid;

fn expected(target: RevisionTarget, revision: u64) -> ExpectedRevision {
  ExpectedRevision::new(target, revision).expect("valid revision")
}

#[test]
fn expected_revisions_wire_shape_of_every_kind() {
  let uuid = Uuid::now_v7();
  for (target, kind) in [
    (RevisionTarget::Project(ProjectId::from_uuid(uuid)), "project"),
    (RevisionTarget::Task(TaskId::from_uuid(uuid)), "task"),
    (RevisionTarget::Module(ModuleId::from_uuid(uuid)), "module"),
    (RevisionTarget::Tag(TagId::from_uuid(uuid)), "tag"),
    (RevisionTarget::Resource(ResourceId::from_uuid(uuid)), "resource"),
    (RevisionTarget::Work(WorkId::from_uuid(uuid)), "work"),
  ] {
    let wire = json!({"kind":kind, "id":uuid.to_string(), "revision":3});
    let revision = expected(target, 3);
    assert_eq!(serde_json::to_value(revision).expect("serialize"), wire);
    assert_eq!(
      serde_json::from_value::<ExpectedRevision>(wire).expect("read"),
      revision
    );
  }
}

#[test]
fn expected_revisions_refuse_zero_out_of_range_and_unknown_shapes() {
  let task = RevisionTarget::Task(TaskId::generate());
  assert_eq!(
    ExpectedRevision::new(task, 0),
    Err(ExpectedRevisionError::OutOfRange(0))
  );
  let above = i64::MAX as u64 + 1;
  assert_eq!(
    ExpectedRevision::new(task, above),
    Err(ExpectedRevisionError::OutOfRange(above))
  );
  assert_eq!(expected(task, i64::MAX as u64).revision(), i64::MAX as u64);
  let id = TaskId::generate();
  for invalid in [
    json!({"kind":"task", "id":id, "revision":0}),
    json!({"kind":"task", "id":id, "revision":-1}),
    json!({"kind":"task", "id":id}),
    json!({"kind":"task", "id":id, "revision":1, "title":"x"}),
    json!({"kind":"run", "id":id, "revision":1}),
    json!({"kind":"task", "id":"7", "revision":1}),
  ] {
    assert!(serde_json::from_value::<ExpectedRevision>(invalid).is_err());
  }
}

#[test]
fn expected_revisions_refuse_a_record_named_twice() {
  let task = RevisionTarget::Task(TaskId::generate());
  let duplicate = ExpectedRevisions::new(vec![expected(task, 1), expected(task, 2)]);
  assert_eq!(duplicate, Err(ExpectedRevisionError::Duplicate(task)));
  let id = Uuid::now_v7();
  let same_id_other_kinds = ExpectedRevisions::new(vec![
    expected(RevisionTarget::Task(TaskId::from_uuid(id)), 1),
    expected(RevisionTarget::Module(ModuleId::from_uuid(id)), 1),
  ])
  .expect("distinct kinds");
  assert_eq!(same_id_other_kinds.as_slice().len(), 2);
  assert!(ExpectedRevisions::new(Vec::new()).expect("empty").as_slice().is_empty());
  let wire = json!([{"kind":"tag","id":id,"revision":1},{"kind":"tag","id":id,"revision":1}]);
  assert!(serde_json::from_value::<ExpectedRevisions>(wire).is_err());
}

#[test]
fn expected_revisions_operation_and_work_ids_are_uuid_strings() {
  let uuid = Uuid::now_v7();
  let operation = OperationId::from_uuid(uuid);
  let work = WorkId::from_uuid(uuid);
  assert_eq!(
    serde_json::to_value(operation).expect("operation"),
    json!(uuid.to_string())
  );
  assert_eq!(serde_json::to_value(work).expect("work"), json!(uuid.to_string()));
  assert_eq!(
    serde_json::from_value::<OperationId>(json!(uuid.to_string())).expect("read"),
    operation
  );
  assert!(serde_json::from_value::<WorkId>(json!("work-1")).is_err());
  assert_ne!(OperationId::generate().as_uuid(), OperationId::generate().as_uuid());
}
