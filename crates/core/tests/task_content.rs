//! A change of a task's content that met a newer revision (design 10.41, decision 17): only fields
//! both people changed differently are the person's to decide; the other person's other changes are
//! kept, and the person's own changes go on top.
//!
//! `cargo test -p orchester-core --test task_content content_`

use std::time::Duration;

use orchester_core::{
  id::ResourceId,
  name::Name,
  task::{
    TaskDetails, TaskLimits,
    content::{Choice, ContentField, ContentMerge, FieldChange},
  },
  tokens::TokenBudget,
};

fn base() -> TaskDetails {
  TaskDetails {
    title: Name::new("Token usage export to CSV").expect("title"),
    brief: "Export token usage.".into(),
    acceptance_criteria: vec!["The export covers the selected period".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(
      TaskLimits::new(TokenBudget::new(1_400_000).expect("budget"), Duration::from_secs(1_800)).expect("limits"),
    ),
  }
}

#[test]
fn content_fields_only_one_side_changed_merge_without_asking() {
  let base = base();
  let mine = TaskDetails {
    title: Name::new("Usage export to CSV").expect("title"),
    ..base.clone()
  };
  let mut theirs = base.clone();
  theirs
    .acceptance_criteria
    .push("The file opens in a spreadsheet without import settings".into());
  let merge = ContentMerge::new(base, mine.clone(), theirs.clone());
  assert!(merge.is_clean());
  assert_eq!(merge.kept(), vec![ContentField::Criteria]);
  assert_eq!(merge.change(ContentField::Title), FieldChange::Mine);
  assert_eq!(merge.change(ContentField::Criteria), FieldChange::Theirs);
  assert_eq!(merge.change(ContentField::Brief), FieldChange::Unchanged);
  let merged = merge.merged(|_| Choice::Theirs);
  assert_eq!(merged.title, mine.title);
  assert_eq!(merged.acceptance_criteria, theirs.acceptance_criteria);
}

#[test]
fn content_fields_both_changed_are_the_person_s_choice() {
  let base = base();
  let mine = TaskDetails {
    brief: "Export the token usage of a period to CSV.".into(),
    ..base.clone()
  };
  let theirs = TaskDetails {
    brief: "Export token usage to CSV from the Usage dashboard.".into(),
    limits: None,
    ..base.clone()
  };
  let merge = ContentMerge::new(base, mine.clone(), theirs.clone());
  assert_eq!(merge.conflicts(), vec![ContentField::Brief]);
  assert_eq!(merge.kept(), vec![ContentField::Limits]);
  let yours = merge.merged(|_| Choice::Mine);
  assert_eq!((yours.brief.as_str(), yours.limits), (mine.brief.as_str(), None));
  let hers = merge.merged(|_| Choice::Theirs);
  assert_eq!(hers, theirs);
}

#[test]
fn content_fields_changed_the_same_way_do_not_conflict() {
  let base = base();
  let same = TaskDetails {
    pipeline: None,
    ..base.clone()
  };
  let merge = ContentMerge::new(base, same.clone(), same.clone());
  assert!(merge.is_clean());
  assert!(merge.kept().is_empty());
  assert_eq!(merge.merged(|_| Choice::Mine), same);
}
