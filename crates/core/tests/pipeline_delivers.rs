//! What a pipeline's runs deliver (ORC-93): changes, a report, work outside the repository, or changes or a report;
//! pipelines saved before it existed deliver changes, and a run that committed nothing completes or is flagged as what
//! it delivers says. Run with `cargo test -p orchester-core --test pipeline_delivers delivers`.

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{
    Deliverable, Pipeline, PipelineChange, PipelineEdit, PromptText, RepositoryAccess, Step, StepKey, StepKind,
    WithoutCommits, changes,
  },
};

fn investigate() -> Pipeline {
  Pipeline::new(vec![Step {
    key: StepKey::new("investigate").expect("valid key"),
    name: Name::new("Investigate").expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Find the cause.").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline")
}

#[test]
fn delivers_a_pipeline_saved_without_it_delivers_changes_and_a_choice_is_kept() {
  let stored = serde_json::to_value(investigate()).expect("serialize");
  assert!(
    !stored.to_string().contains("delivers"),
    "changes is not written: {stored}"
  );
  let read: Pipeline = serde_json::from_value(stored).expect("an older pipeline reads");
  assert_eq!(read.delivers(), Deliverable::Changes);

  for delivers in [Deliverable::Report, Deliverable::Outside, Deliverable::ChangesOrReport] {
    let stored = serde_json::to_value(investigate().with_delivers(delivers)).expect("serialize");
    assert_eq!(stored["delivers"], delivers.as_str(), "{stored}");
    let read: Pipeline = serde_json::from_value(stored).expect("reads");
    assert_eq!(read.delivers(), delivers);
  }
}

#[test]
fn delivers_names_read_back_and_unknown_ones_are_refused() {
  for delivers in Deliverable::ALL {
    assert_eq!(delivers.as_str().parse::<Deliverable>(), Ok(delivers));
  }
  assert_eq!(
    ["changes", "report", "outside", "changes_or_report"],
    Deliverable::ALL.map(Deliverable::as_str)
  );
  assert!("commits".parse::<Deliverable>().is_err());
}

#[test]
fn delivers_only_changes_flags_a_run_that_committed_nothing() {
  assert_eq!(Deliverable::Changes.without_commits(), WithoutCommits::Flagged);
  for delivers in [Deliverable::Report, Deliverable::Outside, Deliverable::ChangesOrReport] {
    assert_eq!(delivers.without_commits(), WithoutCommits::Completes, "{delivers}");
  }
}

#[test]
fn delivers_the_task_choice_wins_over_the_pipeline() {
  assert_eq!(Deliverable::for_task(None, Deliverable::Report), Deliverable::Report);
  assert_eq!(
    Deliverable::for_task(Some(Deliverable::Changes), Deliverable::Report),
    Deliverable::Changes
  );
}

#[test]
fn delivers_an_edit_keeps_it_and_the_publish_review_lists_its_change() {
  let published = investigate();
  let mut edit = PipelineEdit::new(published.clone());
  assert_eq!(edit.delivers(), Deliverable::Changes);
  edit.set_delivers(Deliverable::Outside);
  let draft = edit.validate().expect("valid");
  assert_eq!(draft.delivers(), Deliverable::Outside);
  assert_eq!(PipelineEdit::new(draft.clone()).delivers(), Deliverable::Outside);
  assert_eq!(
    changes(&published, &draft),
    [PipelineChange::Delivers {
      from: Deliverable::Changes,
      to: Deliverable::Outside
    }]
  );
}
